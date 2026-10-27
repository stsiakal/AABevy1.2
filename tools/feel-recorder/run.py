#!/usr/bin/env python3
"""Feel recorder: run one scenario in the original 1.2 client and in Bevy, record both.

One command:

    python tools/feel-recorder/run.py --scenario mounted

It starts the move-golden lane stack with the movement field trace, launches the hidden
original client and (after it) the headless/offscreen Bevy client, drives the same timed
scenario through each client's own input path, records per-frame channels, the server's
movement packets and a frame strip, and writes CSVs, overlay plots and metrics under
`shots/feel-recorder/<scenario>-<stamp>/`.

No OS input is sent: the original is driven through aaemu-clientdriver (Lua + client_input),
Bevy through its script runner. Passwords are read from a temporary file that is deleted at
the end.
"""

import argparse
import ctypes
import importlib.util
import json
import math
import os
import re
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
from ctypes import wintypes
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_slots  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUT_ROOT = ROOT / "shots" / "feel-recorder"
LANE = Path(os.environ.get("FEEL_LANE", "D:/aaemu-wt/move-golden"))
LIVE = Path(os.environ.get("FEEL_LIVE", "D:/aaemu-live-move-golden"))
LOGIN = os.environ.get("FEEL_LOGIN", "127.0.0.1:28237")
CLIENT = Path(os.environ.get("FEEL_CLIENT", "D:/AA1.2/bin32/archeage.exe"))
LAUNCHER = Path(os.environ.get("FEEL_LAUNCHER", str(LIVE / "bin" / "aaemu-launcher.exe")))
DRIVER = Path(os.environ.get("FEEL_DRIVER", "D:/aaemu-target/move-golden/debug/aaemu-clientdriver.exe"))
DLL = Path(os.environ.get("FEEL_DLL", "D:/aaemu-target/move-golden/aadriver/aadriver.dll"))
PASSWORD_SOURCE = Path(os.environ.get("FEEL_PASSWORD", str(LIVE / "cap-pw.txt")))
BEVY_TARGET = Path(os.environ.get("FEEL_BEVY_TARGET", "D:/aaemu-target/bevy-pr"))
BEVY_EXE = BEVY_TARGET / "debug" / "aa-client.exe"
SERVER_LOG = Path(os.environ.get("FEEL_SERVER_LOG", str(LIVE / "logs" / "rs-game.log")))
CAMERA_GLOBAL_RVA = 0xB0E2B8


def log(*a):
    print("feel:", *a, flush=True)


# ---------------------------------------------------------------- MCP / driver

class Driver:
    """The original client's MCP stdio server (one request at a time, lock protected)."""

    def __init__(self, command):
        self.lock = threading.RLock()
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        text=True, encoding="utf-8")
        self.sequence = 0
        self.call("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                 "clientInfo": {"name": "feel-recorder", "version": "1"}})
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, payload):
        self.process.stdin.write(json.dumps(payload) + "\n")
        self.process.stdin.flush()

    def call(self, method, params=None):
        with self.lock:
            self.sequence += 1
            request = {"jsonrpc": "2.0", "id": self.sequence, "method": method}
            if params is not None:
                request["params"] = params
            self._send(request)
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError("driver MCP closed stdout")
            response = json.loads(line)
            if response.get("id") != self.sequence:
                raise RuntimeError("driver MCP response id mismatch")
            if "error" in response:
                raise RuntimeError(str(response["error"]))
            return response["result"]

    def tool(self, name, **arguments):
        with self.lock:
            result = self.call("tools/call", {"name": name, "arguments": arguments})
            if result.get("isError"):
                raise RuntimeError("\n".join(c.get("text", "") for c in result["content"]))
            return result

    def text(self, name, **arguments):
        with self.lock:
            return "\n".join(c["text"] for c in self.tool(name, **arguments)["content"] if c["type"] == "text")

    def screenshot(self, path, pid):
        with self.lock:
            content = self.tool("client_screenshot", pid=pid)["content"]
            import base64
            image = next(c for c in content if c["type"] == "image")
            path.write_bytes(base64.b64decode(image["data"], validate=True))

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=15)
        except Exception:
            pass


# ---------------------------------------------------------------- lane stack

def port_open(port=None):
    port = port or int(LOGIN.rsplit(":", 1)[1])
    with socket.socket() as s:
        s.settimeout(0.3)
        return s.connect_ex(("127.0.0.1", port)) == 0


def stack_up():
    # the game listener is enough: the login listener alone does not mean a game server
    return port_open(int(LOGIN.rsplit(":", 1)[1]) + 2)


def stack_run(script, timeout=300, env=None):
    full_env = os.environ.copy()
    full_env.update({"AAEMU_LANE": LANE.name})
    if env:
        full_env.update(env)
    subprocess.run(["bash", str(LANE / "Scripts" / "live-stack.sh"), script], cwd=LANE,
                   env=full_env, check=True, timeout=timeout)


def ensure_stack(reuse):
    if reuse and stack_up():
        log("lane stack already up (--reuse-stack)")
        return
    if stack_up():
        log("restarting the lane stack with the movement field trace")
        stack_run("stop", timeout=90)
    log("starting the lane stack (movement packet trace on)")
    stack_run("start", env={"RUST_LOG": "info,aaemu_game::net::fields=trace"})


# ---------------------------------------------------------------- scenarios

def load_scenario(name):
    path = Path(__file__).resolve().parent / "scenarios" / f"{name}.json"
    return json.loads(path.read_text(encoding="utf-8"))


def scenario_end(scenario):
    return max((e.get("until", e["at"]) for e in scenario["events"]), default=0.0)


# ---------------------------------------------------------------- original client

SAMPLE_LUA = r"""
local row = {}
local e = _G.FEEL_PE
if e == nil then return 'noent' end
local ok,p = pcall(function() return e:GetWorldPos() end)
if not ok or p == nil then return 'nopos' end
local ok2,a = pcall(function() return e:GetAngles() end)
local ok3,v = pcall(function() return e:GetVelocity() end)
local ok4,b = pcall(function() return e:GetBonePos('Bip01 Pelvis',0) end)
local ok5,d = pcall(function() return e:GetBoneDir('Bip01 Pelvis',0) end)
if not ok2 then a = {x=0,y=0,z=0} end
if not ok3 then v = {x=0,y=0,z=0} end
local ps = '?'
pcall(function() local s = e.actor:GetCurrentAnimationState(); if s then ps = s end end)
-- CryEntity script binding: GetAnimationTime(nSlot, nLayer), seconds.
local function anim_ms(x)
  local ok,t = pcall(function() return x:GetAnimationTime(0, 0) end)
  if ok and type(t) == 'number' then return string.format('%.1f', t * 1000.0) end
  return ''
end
local function vec(t) if type(t) ~= 'table' then return 'nil' end
  return string.format('%.5f,%.5f,%.5f', t.x or 0, t.y or 0, t.z or 0) end
local h = _G.FEEL_HE
local hp,ha,hv,hs = 'nil','nil','nil',''
local hms = ''
local hbd,hbn = 'nil',''
if h ~= nil then
  local o1,p1 = pcall(function() return h:GetWorldPos() end)
  local o2,a1 = pcall(function() return h:GetAngles() end)
  local o3,v1 = pcall(function() return h:GetVelocity() end)
  if o1 and p1 then hp = vec(p1) end
  if o2 and a1 then ha = vec(a1) end
  if o3 and v1 then hv = vec(v1) end
  pcall(function() local s = h.actor:GetCurrentAnimationState(); if s then hs = s end end)
  hms = anim_ms(h)
  -- the mount's root bone (Bip01 when present) for the visual yaw
  for _,n in ipairs({'Bip01','Bip01 Pelvis','bone_spine_driver'}) do
    local okd,dd = pcall(function() return h:GetBoneDir(n,0) end)
    if okd and type(dd) == 'table' then hbd = vec(dd); hbn = n; break end
  end
end
return table.concat({
  tostring(System.GetFrameID()),
  string.format('%.6f', System.GetCurrTime()),
  vec(p), vec(a), vec(v),
  hp, ha, hv,
  ok4 and vec(b) or 'nil', ok5 and vec(d) or 'nil',
  ps, hs, anim_ms(e), hms, hbd, hbn,
}, '|')
"""

INSTALL_LUA = r"""
_G.FEEL_RUN = false
_G.FEEL_STOP = false
_G.FEEL_ROWS = {}
local function sample()
%s
end
local function tick()
  if _G.FEEL_STOP then return end
  if _G.FEEL_RUN then
    local ok, row = pcall(sample)
    if ok and type(row) == 'string' and row ~= 'noent' and row ~= 'nopos' then
      _G.FEEL_ROWS[#_G.FEEL_ROWS + 1] = row
    end
  end
  Script.SetTimer(16, tick)
end
Script.SetTimer(16, tick)
return 'installed'
""" % SAMPLE_LUA



class ProcessMemory:
    def __init__(self, pid):
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.handle = self.kernel.OpenProcess(0x0410, False, pid)  # QUERY_INFORMATION | VM_READ
        if not self.handle:
            raise RuntimeError("OpenProcess failed")
        self.pid = pid

    def read(self, addr, size):
        buf = (ctypes.c_char * size)()
        got = ctypes.c_size_t()
        if not self.kernel.ReadProcessMemory(self.handle, ctypes.c_void_p(addr), buf, size, ctypes.byref(got)):
            return None
        return bytes(buf[:got.value])

    def u32(self, addr):
        b = self.read(addr, 4)
        return struct.unpack("<I", b)[0] if b and len(b) == 4 else 0

    def f32s(self, addr, n):
        b = self.read(addr, 4 * n)
        return struct.unpack("<%df" % n, b) if b and len(b) == 4 * n else None

    def module_base(self, name):
        TH32CS_SNAPMODULE = 0x8
        TH32CS_SNAPMODULE32 = 0x10

        class ME(ctypes.Structure):
            _fields_ = [("dwSize", wintypes.DWORD), ("th32ModuleID", wintypes.DWORD),
                        ("th32ProcessID", wintypes.DWORD), ("GlblcntUsage", wintypes.DWORD),
                        ("ProccntUsage", wintypes.DWORD), ("modBaseAddr", ctypes.c_void_p),
                        ("modBaseSize", wintypes.DWORD), ("hModule", wintypes.HMODULE),
                        ("szModule", ctypes.c_wchar * 256), ("szExePath", ctypes.c_wchar * 260)]
        snap = self.kernel.CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, self.pid)
        me = ME()
        me.dwSize = ctypes.sizeof(ME)
        ok = self.kernel.Module32FirstW(snap, ctypes.byref(me))
        while ok:
            if me.szModule.lower() == name.lower():
                self.kernel.CloseHandle(snap)
                return me.modBaseAddr or 0
            ok = self.kernel.Module32NextW(snap, ctypes.byref(me))
        self.kernel.CloseHandle(snap)
        return 0


class OriginalClient:
    def __init__(self, driver, password_file, user, character, artifact_dir):
        self.d = driver
        self.pid = None
        self.user = user
        self.character = character
        self.password_file = password_file
        self.artifact_dir = artifact_dir

    def lua(self, code, target="system"):
        return self.d.text("client_lua", pid=self.pid, target=target, code=code)

    def ui(self, code):
        return self.lua(code, "ui")

    def chat(self, message):
        return self.d.text("client_chat", pid=self.pid, message=message)

    def key(self, key, ms):
        return self.d.text("client_input", pid=self.pid, key=key, duration_ms=int(ms))

    def release(self):
        return self.d.text("client_input", pid=self.pid, key="release")

    def wait_stage(self, stage, timeout):
        deadline = time.monotonic() + timeout
        last = ""
        while time.monotonic() < deadline:
            try:
                last = self.ui("return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()")
                if last == str(stage):
                    return
            except Exception as e:
                last = str(e)
            time.sleep(0.5)
        raise RuntimeError(f"original stage {stage} not reached: {last}")

    def wait_player(self, timeout):
        deadline = time.monotonic() + timeout
        last = ""
        while time.monotonic() < deadline:
            try:
                state = self.ui("return X2LoginCharacter:GetCurrentStage(), X2Unit and X2Unit:UnitName('player')")
                stage, _, name = state.partition("	")
                if stage == "0" and name not in ("", "nil"):
                    return
                last = state
            except Exception as e:
                last = str(e)
            time.sleep(0.5)
        raise RuntimeError("original player did not appear: " + last)

    def launch(self):
        launch = self.d.text("client_launch", fast_start=True, fast_pak=True, hidden=True,
                             user=self.user, password_file=str(self.password_file))
        m = re.search(r"pid (\d+)", launch)
        if not m:
            raise RuntimeError("client_launch: " + launch)
        self.pid = int(m.group(1))
        log("original client pid", self.pid)
        self.wait_stage(3, 200)
        self.ui("X2World:EnterWorld(1,0)")
        self.wait_stage(4, 200)
        self.ui("local b=characterSelectWindow.bottomPanel.startButton b:OnClick('LeftButton')")
        self.wait_player(200)
        time.sleep(2.5)

    def place(self, start):
        self.chat("/move %.1f %.1f %.1f" % (start["x"], start["y"], start["z"]))
        time.sleep(2.2)

    def entities(self, with_mount=True):
        code = r"""
local pe, he, best
for _, x in pairs(System.GetEntities() or {}) do
  local n = x:GetName()
  if n == CHAR_NAME then pe = x end
end
if pe ~= nil then
  local pp = pe:GetWorldPos()
  for _, x in pairs(System.GetEntities() or {}) do
    local n = string.lower(x:GetName())
    if string.find(n, 'horse') and x ~= pe then
      local p = x:GetWorldPos()
      local dx, dy, dz = p.x - pp.x, p.y - pp.y, p.z - pp.z
      local d = dx*dx + dy*dy + dz*dz
      if d < 400 and (best == nil or d < best) then best = d; he = x end
    end
  end
end
_G.FEEL_PE = pe
_G.FEEL_HE = WITH_MOUNT and he or nil
return tostring(pe ~= nil) .. '|' .. tostring(_G.FEEL_HE ~= nil)
""".replace("WITH_MOUNT", "true" if with_mount else "false").replace("CHAR_NAME", json.dumps(self.character))
        return self.lua(code)

    def ensure_item(self, item):
        self.chat("/item add self %d" % item)
        time.sleep(0.8)

    def summon(self, item, timeout=25):
        self.entities()
        if self.ui("return tostring(X2Mate:IsPlayerPetExists())").strip() == "true":
            self.ui("X2Mate:DismissPet() return 1")
            time.sleep(3.5)
        slot = None
        try:
            inv = json.loads(self.d.text("client_inventory", pid=self.pid))
            slot = next((int(k) for k, v in inv["slots"].items()
                         if v.get("item_template_id") == item and not v.get("dead")), None)
        except Exception:
            pass
        if slot is not None:
            self.ui("X2Bag:HandleUse(%d) return 1" % slot)
        else:
            self.ui("return X2Bag:UseItemByType(%d)" % item)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.ui("return tostring(X2Mate:IsPlayerPetExists())").strip() == "true":
                time.sleep(0.8)
                self.entities()
                return
            time.sleep(0.5)
        raise RuntimeError("original summon: no pet")

    def mount(self, timeout=20):
        self.entities()
        self.ui("X2Mate:MountMate() return 1")
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.ui("return tostring(X2Mate:IsPlayerMounting())").strip() == "true":
                time.sleep(1.0)
                self.entities()
                return
            time.sleep(0.4)
        raise RuntimeError("original mount: not seated")

    def heading(self):
        v = self.lua("local e=_G.FEEL_HE if e==nil then e=_G.FEEL_PE end if e==nil then return '0' end "
                     "local a=e:GetAngles() return string.format('%.6f',a.z)")
        try:
            return float(v)
        except ValueError:
            return 0.0

    def set_heading(self, deg):
        target = math.radians(deg)
        for _ in range(30):
            now = self.heading()
            d = (target - now + math.pi) % (2 * math.pi) - math.pi
            if abs(d) < math.radians(2.5):
                return
            ms = int(min(1200, max(70, abs(math.degrees(d)) / 90.0 * 1000)))
            self.key("a" if d > 0 else "d", ms)
            time.sleep(ms / 1000.0 + 0.25)
        raise RuntimeError("original heading not reached")

    def install_timer(self):
        return self.lua(INSTALL_LUA)

    def start_rows(self):
        return self.lua("_G.FEEL_RUN = true return 'run'")

    def stop_rows(self):
        return self.lua("_G.FEEL_STOP = true _G.FEEL_RUN = false return #_G.FEEL_ROWS")

    def fetch_rows(self, chunk=400):
        total = int(self.lua("return #_G.FEEL_ROWS").strip())
        rows = []
        for start in range(1, total + 1, chunk):
            end = min(total, start + chunk - 1)
            text = self.lua("return table.concat(_G.FEEL_ROWS, '\\n', %d, %d)" % (start, end))
            if text:
                rows.extend(text.splitlines())
        return rows


class CameraSampler(threading.Thread):
    def __init__(self, pid, interval=0.02):
        super().__init__(daemon=True)
        self.pid = pid
        self.interval = interval
        self.stop_event = threading.Event()
        self.rows = []
        self.error = None

    def run(self):
        try:
            mem = ProcessMemory(self.pid)
            base = mem.module_base("x2game.dll")
            if not base:
                raise RuntimeError("x2game.dll not loaded")
            global_addr = base + CAMERA_GLOBAL_RVA
            while not self.stop_event.is_set():
                cam = mem.u32(global_addr)
                if cam:
                    # CameraState (port/camera/state.rs): +0x3c view target, +0x64 final eye.
                    pos = mem.f32s(cam + 0x64, 3)
                    target = mem.f32s(cam + 0x3C, 3)
                    if pos and target and any(abs(x) > 1e-4 for x in pos):
                        d = [target[i] - pos[i] for i in range(3)]
                        n = math.sqrt(sum(x * x for x in d)) or 1.0
                        self.rows.append((time.time() * 1000.0, *pos, *(x / n for x in d)))
                self.stop_event.wait(self.interval)
        except Exception as e:
            self.error = str(e)

    def stop(self):
        self.stop_event.set()
        self.join(timeout=5)
        return self.rows


def merged_camera(camera_rows, wall_ms):
    if not camera_rows:
        return None
    # nearest sample (the camera runs at ~50 Hz, rows at ~20 Hz)
    return min(camera_rows, key=lambda r: abs(r[0] - wall_ms))


def run_original(scenario, args, out):
    orig_dir = out / "original"
    orig_dir.mkdir(parents=True, exist_ok=True)
    command = [str(DRIVER), "mcp", "--dll", str(DLL), "--client", str(CLIENT),
               "--launcher", str(LAUNCHER), "--password-file", str(args.password_file),
               "--login", LOGIN, "--user", args.original_user,
               "--artifact-dir", str(orig_dir / "mcp")]
    d = Driver(command)
    client = OriginalClient(d, args.password_file, args.original_user, args.original_character, orig_dir)
    frames = []
    frame_stop = threading.Event()
    camera = None
    try:
        client.launch()
        try:
            client.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass
        client.place(scenario["start"])
        if scenario.get("mount"):
            client.ensure_item(scenario["item"])
            client.summon(scenario["item"])
            client.mount()
            client.set_heading(scenario["start"].get("yaw_deg", 0.0))
        else:
            client.entities(with_mount=False)
            client.release()
        client.install_timer()
        camera = CameraSampler(client.pid)
        camera.start()

        def capture_frames():
            i = 0
            while not frame_stop.is_set():
                path = orig_dir / f"o_{i:04d}.png"
                stamp = time.time() * 1000.0
                try:
                    client.d.screenshot(path, client.pid)
                    frames.append({"wall_ms": stamp, "file": path.name})
                except Exception as e:
                    log("original frame skipped:", str(e)[:120])
                i += 1
                frame_stop.wait(args.frame_interval)

        frame_thread = None
        if not args.no_frames:
            frame_thread = threading.Thread(target=capture_frames, daemon=True)
            frame_thread.start()

        wall0 = time.time()
        client.start_rows()
        log("original scenario starts (t=0)")
        for event in scenario["events"]:
            delay = event["at"] - (time.time() - wall0)
            if delay > 0:
                time.sleep(delay)
            keys = event.get("keys", [])
            if keys:
                # A timed key can still be down when the next event starts (the driver's release
                # runs on the game's main thread); the scenario has gaps between events, so clear
                # the chord first.
                client.release()
                duration = max(0.05, event.get("until", event["at"]) - event["at"]) * 1000.0
                try:
                    client.release()
                except Exception:
                    pass
                for key in keys:
                    try:
                        client.key(key, duration)
                    except Exception:
                        client.release()
                        client.key(key, duration)
        tail = scenario_end(scenario) + 1.5 - (time.time() - wall0)
        if tail > 0:
            time.sleep(tail)
        client.stop_rows()
        frame_stop.set()
        if frame_thread is not None:
            frame_thread.join(timeout=5)
        wall1 = time.time()
        camera_rows = camera.stop()
        rows = client.fetch_rows()
        log("original rows", len(rows), "frames", len(frames), "camera", len(camera_rows))
        (orig_dir / "raw_rows.txt").write_text("\n".join(rows), encoding="utf-8")
        (orig_dir / "meta.json").write_text(json.dumps({
            "wall_start_ms": wall0 * 1000.0, "wall_end_ms": wall1 * 1000.0,
            "start": scenario["start"], "frames": frames,
            "camera_error": camera.error,
        }, indent=1), encoding="utf-8")
        write_camera_csv(orig_dir / "camera.csv", camera_rows)
        return parse_original_rows(rows, wall0)
    finally:
        try:
            if client.pid:
                client.release()
        except Exception:
            pass
        if camera:
            camera.stop()
        if not args.keep_clients and client.pid:
            try:
                d.text("client_stop", pid=client.pid)
            except Exception:
                pass
        d.close()


def write_camera_csv(path, rows):
    with path.open("w", encoding="utf-8", newline="") as f:
        f.write("wall_ms,x,y,z,dx,dy,dz\n")
        for r in rows:
            f.write("%.3f,%.4f,%.4f,%.4f,%.5f,%.5f,%.5f\n" % r)


def parse_original_rows(rows, wall0):
    out = []
    for text in rows:
        parts = text.split("|")
        if len(parts) < 12:
            continue
        def vec(s):
            if s == "nil":
                return None
            return [float(x) for x in s.split(",")]
        p, a, v = vec(parts[2]), vec(parts[3]), vec(parts[4])
        if not p:
            continue
        m, ma, mv = vec(parts[5]), vec(parts[6]), vec(parts[7])
        pelvis, pdir = vec(parts[8]), vec(parts[9])
        out.append({
            "frame": int(parts[0]),
            "client_t": float(parts[1]),
            "player": p, "player_angles": a or [0, 0, 0], "player_vel": v or [0, 0, 0],
            "mount": m, "mount_angles": ma, "mount_vel": mv,
            "pelvis": pelvis, "pelvis_dir": pdir,
            "player_state": parts[10], "mount_state": parts[11],
            "player_clip_ms": parts[12] if len(parts) > 12 else "",
            "mount_clip_ms": parts[13] if len(parts) > 13 else "",
            "mount_bone_dir": vec(parts[14]) if len(parts) > 14 and parts[14] != "nil" else None,
            "mount_bone_name": parts[15] if len(parts) > 15 else "",
            "wall_ms": wall0 * 1000.0,
        })
    if out:
        t0 = out[0]["client_t"]
        for r in out:
            r["t_ms"] = (r["client_t"] - t0) * 1000.0
            r["wall_ms"] = wall0 * 1000.0 + r["t_ms"]
    return out


# ---------------------------------------------------------------- bevy client

def bevy_script(scenario, out):
    lines = ["feel SETUP", "cursor 0.98 0.02",
             "lua if tutorial and tutorial.window then tutorial.window:Show(false) end"]
    if scenario.get("mount"):
        lines += [
            "lua if X2Mate:IsPlayerPetExists() then X2Mate:DismissPet() end",
            "wait 2",
            f"gm item add self {scenario['item']}",
            "wait 5",
            f"use {scenario['item']}",
            "wait 12",
            "lua X2Mate:MountMate()",
            "waitfor 30 riding",
            "wait 4",
        ]
    else:
        lines += ["wait 1"]
    lines.append("feel START")
    cursor = 0.0
    for event in scenario["events"]:
        at = event["at"]
        if at > cursor:
            lines.append("wait %.3f" % (at - cursor))
        for key in event.get("keys", []):
            lines.append(f"keydown {key}")
        until = event.get("until", at)
        if until > at:
            lines.append("wait %.3f" % (until - at))
        for key in event.get("keys", []):
            lines.append(f"keyup {key}")
        cursor = until
    lines += ["feel END", "wait 0.3", "quit now"]
    path = out / "bevy-script.txt"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def run_bevy(scenario, args, out):
    bevy_dir = out / "bevy"
    bevy_dir.mkdir(parents=True, exist_ok=True)
    script = bevy_script(scenario, out)
    csv_path = bevy_dir / "feel.csv"
    frames_dir = bevy_dir / "frames"
    cmd = [
        str(args.bevy_exe), "--headless", "--mute", "--account", args.bevy_user,
        "--login", LOGIN, "--password-file", str(args.password_file),
        "--character", args.bevy_character,
        "--pos", "%.1f,%.1f,%.1f" % (scenario["start"]["x"], scenario["start"]["y"], scenario["start"]["z"]),
        "--yaw", str(scenario["start"].get("yaw_deg", 0.0)),
        "--script", str(script),
        "--feel-csv", str(csv_path),
        "--feel-fps", "60",
        "--feel-frame-every", str(args.bevy_frame_every),
        "--size", "1280x720",
        "--headless-fps", "120",
        "--offscreen-shot", str(bevy_dir / "initial.png"),
        "--shot-after", "100000",
    ]
    cmd += list(args.bevy_arg)
    if not args.no_frames:
        cmd += ["--feel-frames", str(frames_dir)]
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(BEVY_TARGET)
    env["AA_DB"] = str(LIVE / "rs-game" / "Data" / "compact.sqlite3")
    log_path = out / "bevy.log"
    with log_path.open("w", encoding="utf-8") as logf:
        log("bevy:", " ".join(cmd))
        proc = subprocess.run(cmd, cwd=ROOT, env=env, stdout=logf, stderr=subprocess.STDOUT,
                              timeout=args.bevy_timeout)
    if proc.returncode != 0:
        tail = log_path.read_text(encoding="utf-8", errors="replace").splitlines()[-15:]
        raise RuntimeError("bevy failed (%d):\n%s" % (proc.returncode, "\n".join(tail)))
    return read_bevy_csv(csv_path)


def read_bevy_csv(path):
    import csv
    rows = []
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            def fnum(key):
                try:
                    return float(row[key])
                except (KeyError, ValueError):
                    return 0.0
            rows.append({k: (v if k in ("player_state", "mount_state", "rider_state") else None) for k, v in row.items()})
            r = rows[-1]
            for k, v in row.items():
                if k in ("player_state", "mount_state", "rider_state"):
                    continue
                r[k] = fnum(k)
    return rows


def read_marks(out):
    path = out / "bevy" / "feel.csv.marks.csv"
    marks = {}
    if path.exists():
        import csv
        with path.open(newline="", encoding="utf-8") as f:
            for row in csv.DictReader(f):
                marks.setdefault(row["name"], []).append((float(row["ms"]), float(row["wall_ms"])))
    return marks


# ---------------------------------------------------------------- server trace

MOVE_RE = re.compile(
    r"C->S CSMoveUnitPacket fields: obj_id=(\d+) move_type=Some\((\w+)\) flags=MoveTypeFlags\((\d+)\) "
    r"position=Some\(\(([-\d.]+), ([-\d.]+), ([-\d.]+)\)\) rotation=\(([-\d.]+), ([-\d.]+), ([-\d.]+)\) "
    r"velocity=\(([-\d.]+), ([-\d.]+), ([-\d.]+)\)"
)
LOG_TIME_RE = re.compile(r"^(\d\d):(\d\d):(\d\d)")


def parse_server_trace(start_offset, end_offset):
    if not SERVER_LOG.exists():
        return []
    with SERVER_LOG.open("rb") as f:
        f.seek(start_offset)
        data = f.read(max(0, end_offset - start_offset)).decode("utf-8", "replace")
    today = time.localtime()
    rows = []
    for line in data.splitlines():
        m = MOVE_RE.search(line)
        if not m:
            continue
        tm = LOG_TIME_RE.match(line)
        if not tm:
            continue
        h, mi, se = map(int, tm.groups())
        wall = time.mktime((today.tm_year, today.tm_mon, today.tm_mday, h, mi, se, 0, 0, -1)) * 1000.0
        rows.append({
            "wall_ms": wall, "obj": int(m.group(1)),
            "x": float(m.group(4)), "y": float(m.group(5)), "z": float(m.group(6)),
            "yaw": float(m.group(9)),
        })
    return rows


def server_segment(rows, wall0, wall1, obj):
    return [r for r in rows if r["obj"] == obj and wall0 - 300 <= r["wall_ms"] <= wall1 + 1500]


def main_obj(server, wall0, wall1):
    """The object that sends the most movement in the window: the driven unit (mount when mounted)."""
    import collections
    seg = [p for p in server if wall0 - 500 <= p["wall_ms"] <= wall1 + 2500]
    counts = collections.Counter(p["obj"] for p in seg)
    return counts.most_common(1)[0][0] if counts else None


def detect_obj(server, wall0, wall1, start, exclude=None):
    """The server object id whose first packet is nearest the scenario start."""
    seg = [p for p in server if wall0 - 500 <= p["wall_ms"] <= wall1 + 2500 and p["obj"] != exclude]
    if not seg:
        return None
    best, best_d = None, 1e30
    for p in seg:
        d = (p["x"] - start["x"]) ** 2 + (p["y"] - start["y"]) ** 2
        if d < best_d:
            best_d, best = d, p["obj"]
    return best


def attached_obj(server, wall0, wall1, char_obj):
    """The other object that moves with the character in the window (the mount)."""
    seg = [p for p in server if wall0 - 500 <= p["wall_ms"] <= wall1 + 2500]
    ids = {p["obj"] for p in seg if p["obj"] != char_obj}
    if not ids:
        return None
    # the mount sends the most packets in a mounted scenario and stays close to the rider
    def score(obj):
        own = [p for p in seg if p["obj"] == obj]
        if not own:
            return -1
        ref = [p for p in seg if p["obj"] == char_obj]
        if not ref:
            return -1
        d = min((a["x"] - b["x"]) ** 2 + (a["y"] - b["y"]) ** 2 for a in own for b in ref)
        return len(own) - d * 1e-3
    return max(ids, key=score)


def attach_server(csv_rows, packets):
    if not packets:
        return
    for row in csv_rows:
        wall = row.get("wall_ms", 0.0)
        before = [p for p in packets if p["wall_ms"] <= wall]
        p = before[-1] if before else packets[0]
        row["server_x"], row["server_y"], row["server_z"] = p["x"], p["y"], p["z"]


# ---------------------------------------------------------------- aligned CSV

COLUMNS = [
    "t_ms", "frame", "player_x", "player_y", "player_z", "player_yaw", "player_vx", "player_vy", "player_vz",
    "mount_x", "mount_y", "mount_z", "mount_yaw", "mount_pitch",
    "mount_vx", "mount_vy", "mount_vz", "rider_x", "rider_y", "rider_z", "rider_yaw",
    "rider_bone_yaw", "mount_bone_yaw",
    "cam_x", "cam_y", "cam_z", "cam_dx", "cam_dy", "cam_dz", "cam_yaw", "cam_pitch",
    "player_state", "player_clip_ms", "mount_state", "mount_clip_ms", "rider_state", "rider_clip_ms",
    "server_x", "server_y", "server_z", "server_mount_x", "server_mount_y", "server_mount_z",
]


def write_aligned(path, rows):
    import csv
    with path.open("w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(COLUMNS)
        for r in rows:
            w.writerow([("%.4f" % r[c]) if isinstance(r.get(c), float) else r.get(c, "") for c in COLUMNS])


def camera_angles(dx, dy, dz):
    """Yaw/pitch of a direction, same convention as the state yaws (0 = +y)."""
    try:
        dx, dy, dz = float(dx), float(dy), float(dz)
    except (TypeError, ValueError):
        return "", ""
    return math.atan2(dx, dy), math.atan2(dz, math.hypot(dx, dy))


def shift_original_world(out):
    """GetWorldPos is zone-local in the original (here +6144/+11264); map it onto the
    server/Bevy world.  The median of server-local over the run rejects the one-packet
    lag at the start and during speed changes."""
    dx, dy = [], []
    for r in out:
        if isinstance(r.get("server_x"), float) and isinstance(r.get("player_x"), float):
            dx.append(r["server_x"] - r["player_x"])
            dy.append(r["server_y"] - r["player_y"])
    if not dx:
        return None
    import statistics
    off = (statistics.median(dx), statistics.median(dy))
    for r in out:
        for kx, ky in (("player_x", "player_y"), ("mount_x", "mount_y"),
                       ("rider_x", "rider_y"), ("cam_x", "cam_y")):
            if isinstance(r.get(kx), float):
                r[kx] += off[0]
            if isinstance(r.get(ky), float):
                r[ky] += off[1]
    return off


def _number(text):
    try:
        return float(text)
    except (TypeError, ValueError):
        return ""


def _wrap_angle(a):
    return (a + math.pi) % (2 * math.pi) - math.pi


def _bone_yaw(direction):
    """GetBoneDir / a Cry bone's local +Y, in the game yaw convention (0 = +y)."""
    if not direction or len(direction) < 2:
        return ""
    return math.atan2(-float(direction[0]), float(direction[1]))


def _calibrate_bone_yaw(rows, entity_key, bone_key):
    """The first mounted row is the scenario start (yaw 0): remove the bone's bind-pose offset."""
    for r in rows:
        a, b = r.get(entity_key), r.get(bone_key)
        if isinstance(a, (int, float)) and isinstance(b, (int, float)):
            return _wrap_angle(float(a) - float(b))
    return 0.0


def _apply_bone_yaw(rows, entity_key, bone_key):
    off = _calibrate_bone_yaw(rows, entity_key, bone_key)
    for r in rows:
        b = r.get(bone_key)
        if isinstance(b, (int, float)):
            r[bone_key] = _wrap_angle(float(b) + off)
    return off


def original_channel_rows(raw, camera_rows, server, char_id, mount_id=None):
    out = []
    for r in raw:
        cam = merged_camera(camera_rows, r["wall_ms"]) if camera_rows else None
        m = r["mount"]
        cam_yaw, cam_pitch = camera_angles(cam[4], cam[5], cam[6]) if cam else ("", "")
        out.append({
            "t_ms": r["t_ms"], "frame": r["frame"],
            "player_x": r["player"][0], "player_y": r["player"][1], "player_z": r["player"][2],
            "player_yaw": r["player_angles"][2],
            "player_vx": r["player_vel"][0], "player_vy": r["player_vel"][1], "player_vz": r["player_vel"][2],
            "mount_x": m[0] if m else "", "mount_y": m[1] if m else "", "mount_z": m[2] if m else "",
            "mount_yaw": r["mount_angles"][2] if r["mount_angles"] else "",
            "mount_pitch": r["mount_angles"][1] if r["mount_angles"] else "",
            "mount_vx": r["mount_vel"][0] if r["mount_vel"] else "",
            "mount_vy": r["mount_vel"][1] if r["mount_vel"] else "",
            "mount_vz": r["mount_vel"][2] if r["mount_vel"] else "",
            "rider_x": r["pelvis"][0] if r["pelvis"] else "", "rider_y": r["pelvis"][1] if r["pelvis"] else "",
            "rider_z": r["pelvis"][2] if r["pelvis"] else "",
            "rider_yaw": r["player_angles"][2],
            "rider_bone_yaw": _bone_yaw(r.get("pelvis_dir")),
            "mount_bone_yaw": _bone_yaw(r.get("mount_bone_dir")),
            "cam_x": cam[1] if cam else "", "cam_y": cam[2] if cam else "", "cam_z": cam[3] if cam else "",
            "cam_dx": cam[4] if cam else "", "cam_dy": cam[5] if cam else "", "cam_dz": cam[6] if cam else "",
            "cam_yaw": cam_yaw, "cam_pitch": cam_pitch,
            "player_state": r["player_state"], "player_clip_ms": _number(r.get("player_clip_ms")),
            "mount_state": r["mount_state"], "mount_clip_ms": _number(r.get("mount_clip_ms")),
            "rider_state": r["player_state"], "rider_clip_ms": _number(r.get("player_clip_ms")),
            "wall_ms": r["wall_ms"],
        })
    if out:
        attach_server(out, server_segment(server, out[0]["wall_ms"], out[-1]["wall_ms"], char_id))
        if mount_id:
            for row in out:
                before = [p for p in server_segment(server, out[0]["wall_ms"], out[-1]["wall_ms"], mount_id) if p["wall_ms"] <= row["wall_ms"]]
                if before:
                    p = before[-1]
                    row["server_mount_x"], row["server_mount_y"], row["server_mount_z"] = p["x"], p["y"], p["z"]
        off = shift_original_world(out)
        if off:
            log("original world offset %.1f,%.1f (zone-local -> server world)" % off)
        if mount_id:
            name = next((r.get("mount_bone_name") for r in raw if r.get("mount_bone_name")), "")
            if name:
                log("original mount yaw bone:", name)
        _apply_bone_yaw(out, "player_yaw", "rider_bone_yaw")
        _apply_bone_yaw(out, "mount_yaw", "mount_bone_yaw")
    return out


def bevy_channel_rows(raw, marks, server, char_id, mount_id):
    start_ms = marks.get("START", [(0.0, 0.0)])[0][0]
    end_ms = marks.get("END", [(raw[-1]["ms"] if raw else 0.0, 0.0)])[0][0]
    out = []
    for r in raw:
        t = r["ms"] - start_ms
        if t < 0 or (end_ms > start_ms and r["ms"] > end_ms + 80):
            continue
        cam_yaw, cam_pitch = camera_angles(r["cam_dx"], r["cam_dy"], r["cam_dz"])
        out.append({
            "t_ms": t, "frame": r["frame"],
            "player_x": r["player_x"], "player_y": r["player_y"], "player_z": r["player_z"],
            "player_yaw": r["player_yaw"],
            "player_vx": r["player_vx"], "player_vy": r["player_vy"], "player_vz": r["player_vz"],
            "mount_x": r["mount_x"], "mount_y": r["mount_y"], "mount_z": r["mount_z"],
            "mount_yaw": r["mount_yaw"], "mount_pitch": r["mount_pitch"],
            "mount_vx": "", "mount_vy": "", "mount_vz": "",
            "rider_x": r["rider_x"], "rider_y": r["rider_y"], "rider_z": r["rider_z"], "rider_yaw": r["rider_yaw"],
            "rider_bone_yaw": r.get("rider_bone_yaw", 0.0), "mount_bone_yaw": r.get("mount_bone_yaw", 0.0),
            "cam_x": r["cam_x"], "cam_y": r["cam_y"], "cam_z": r["cam_z"],
            "cam_dx": r["cam_dx"], "cam_dy": r["cam_dy"], "cam_dz": r["cam_dz"],
            "cam_yaw": cam_yaw, "cam_pitch": cam_pitch,
            "player_state": r["player_state"], "player_clip_ms": r["player_clip_ms"],
            "mount_state": r["mount_state"], "mount_clip_ms": r["mount_clip_ms"],
            "rider_state": r["rider_state"], "rider_clip_ms": r["rider_clip_ms"],
            "wall_ms": r["wall_ms"],
        })
    if out:
        attach_server(out, server_segment(server, out[0]["wall_ms"], out[-1]["wall_ms"], char_id))
        if mount_id:
            mrows = server_segment(server, out[0]["wall_ms"], out[-1]["wall_ms"], mount_id)
            for row in out:
                before = [p for p in mrows if p["wall_ms"] <= row["wall_ms"]]
                if before:
                    p = before[-1]
                    row["server_mount_x"], row["server_mount_y"], row["server_mount_z"] = p["x"], p["y"], p["z"]
        _apply_bone_yaw(out, "player_yaw", "rider_bone_yaw")
        _apply_bone_yaw(out, "mount_yaw", "mount_bone_yaw")
    return out


# ---------------------------------------------------------------- main

def build_bevy(args):
    """Build only this worktree's `aa-client` into its own warm target dir.

    A `--bevy-exe` is always treated as prebuilt: it may come from another branch or
    worktree, and building here would either overwrite it or build the wrong source.
    """
    if args.no_build or args.bevy_exe_given:
        log("using prebuilt Bevy exe", args.bevy_exe)
        return
    log("building aa-client into", BEVY_TARGET)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(BEVY_TARGET)
    subprocess.run(["cargo", "build", "-p", "aa-client", "--bin", "aa-client"], cwd=ROOT, env=env,
                   check=True, timeout=600)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--scenario", default="mounted", choices=["mounted", "on_foot", "all"])
    ap.add_argument("--out", default=None)
    ap.add_argument("--bevy-exe", default=None,
                    help="prebuilt aa-client.exe (skips the build; use for another branch/worktree)")
    ap.add_argument("--bevy-label", default=None,
                    help="name for the prebuilt Bevy column in plots/strips (default: bevy)")
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--reuse-stack", action="store_true", help="do not restart a running lane stack")
    ap.add_argument("--stop-stack", action="store_true", help="stop the lane stack at the end")
    ap.add_argument("--keep-clients", action="store_true")
    ap.add_argument("--only", choices=["both", "original", "bevy"], default="both")
    ap.add_argument("--no-frames", action="store_true")
    ap.add_argument("--no-plots", action="store_true")
    ap.add_argument("--frame-interval", type=float, default=1.0, help="original screenshot interval (s)")
    ap.add_argument("--bevy-frame-every", type=int, default=30, help="Bevy game frames between PNGs")
    ap.add_argument("--slot", default=os.environ.get("FEEL_SLOT"),
                    help="worker slot (wp1..wp6, f7, interaction, combat, spare1..3); "
                         "overrides the account/character options")
    ap.add_argument("--original-user", default="movegold3")
    ap.add_argument("--original-character", default="Movegoldc")
    ap.add_argument("--original-char-id", type=int, default=3)
    ap.add_argument("--bevy-user", default="movegold2")
    ap.add_argument("--bevy-character", default="Movegoldb")
    ap.add_argument("--bevy-char-id", type=int, default=2)
    ap.add_argument("--password-source", default=str(PASSWORD_SOURCE))
    ap.add_argument("--bevy-timeout", type=float, default=420.0)
    ap.add_argument("--bevy-arg", action="append", default=[], help="extra argument for the Bevy client")
    args = ap.parse_args()

    if args.slot:
        lane = live_slots.lane_for_path(str(LANE)) or "move-golden"
        original = live_slots.resolve(args.slot, lane, "a")
        bevy = live_slots.resolve(args.slot, lane, "b")
        args.original_user = original.user
        args.original_character = original.character
        args.original_char_id = original.character_id
        args.bevy_user = bevy.user
        args.bevy_character = bevy.character
        args.bevy_char_id = bevy.character_id
        args.slot_offset = live_slots.slot_index(args.slot) * 8.0
    else:
        args.slot_offset = 0.0

    args.bevy_exe_given = args.bevy_exe is not None
    args.bevy_exe = Path(args.bevy_exe).resolve() if args.bevy_exe_given else BEVY_EXE
    args.bevy_label = re.sub(r"[^A-Za-z0-9._-]+", "_", args.bevy_label or "bevy").strip("_") or "bevy"
    if not args.bevy_exe.is_file():
        ap.error(f"--bevy-exe does not exist: {args.bevy_exe}")

    stamp = time.strftime("%Y%m%d-%H%M%S")
    names = ["mounted", "on_foot"] if args.scenario == "all" else [args.scenario]
    suffix = "" if args.bevy_label == "bevy" else f"-{args.bevy_label}"
    out_root = Path(args.out) if args.out else OUT_ROOT / f"{args.scenario}-{stamp}{suffix}"
    out_root.mkdir(parents=True, exist_ok=True)

    # a temp, git-ignored password file; deleted in the finally block
    pw = out_root / ".password.tmp"
    shutil.copyfile(args.password_source, pw)
    args.password_file = pw
    args.bevy_exe = Path(args.bevy_exe)
    stack_started = False
    try:
        build_bevy(args)
        ensure_stack(args.reuse_stack)
        stack_started = True
        for name in names:
            scenario = load_scenario(name)
            if args.slot_offset:
                scenario["start"]["x"] += args.slot_offset
                scenario["start"]["y"] += args.slot_offset * 0.5
                log(f"slot offset: start moved to {scenario['start']['x']:.1f},"
                    f"{scenario['start']['y']:.1f}")
            out = out_root if len(names) == 1 else out_root / name
            out.mkdir(parents=True, exist_ok=True)
            log(f"=== scenario {name} -> {out} (bevy: {args.bevy_label} {args.bevy_exe})")
            off0 = SERVER_LOG.stat().st_size if SERVER_LOG.exists() else 0
            wall_all0 = time.time() * 1000.0
            orig_raw = run_original(scenario, args, out) if args.only in ("both", "original") else []
            off1 = SERVER_LOG.stat().st_size if SERVER_LOG.exists() else 0
            bevy_raw = run_bevy(scenario, args, out) if args.only in ("both", "bevy") else []
            off2 = SERVER_LOG.stat().st_size if SERVER_LOG.exists() else 0
            server = parse_server_trace(off0, off2)
            log("server movement packets", len(server))
            camera_rows = read_camera_csv(out / "original" / "camera.csv")
            orig_meta = out / "original" / "meta.json"
            orig_wall0 = orig_wall1 = 0.0
            if orig_raw and orig_meta.exists():
                om = json.loads(orig_meta.read_text(encoding="utf-8"))
                orig_wall0, orig_wall1 = om["wall_start_ms"], om["wall_end_ms"]
            orig_char = main_obj(server, orig_wall0, orig_wall1) if orig_raw else None
            if orig_char is None and orig_raw:
                orig_char = detect_obj(server, orig_wall0, orig_wall1, scenario["start"])
            orig = original_channel_rows(orig_raw, camera_rows, server, orig_char or 0, orig_char if scenario.get("mount") else None)
            marks = read_marks(out)
            bevy_char = 0
            mount_id = 0
            if bevy_raw:
                st = marks.get("START", [(0.0, 0.0)])[0]
                en = marks.get("END", [(0.0, 0.0)])[0]
                bevy_char = main_obj(server, st[1], en[1]) or 0
                mount_id = bevy_char if scenario.get("mount") else 0
            bevy = bevy_channel_rows(bevy_raw, marks, server, bevy_char, mount_id)
            aligned = out / "aligned"
            aligned.mkdir(exist_ok=True)
            write_aligned(aligned / "original.csv", orig)
            write_aligned(aligned / "bevy.csv", bevy)
            (aligned / "server.csv").write_text(
                "wall_ms,obj,x,y,z,yaw" + chr(10) + chr(10).join(
                    "%.3f,%d,%.4f,%.4f,%.4f,%.5f" % (p["wall_ms"], p["obj"], p["x"], p["y"], p["z"], p["yaw"])
                    for p in server) + chr(10), encoding="utf-8")
            run_analysis(out, scenario, orig, bevy, server, args)
            log("scenario", name, "done")
        log("output:", out_root)
    finally:
        try:
            pw.unlink()
        except OSError:
            pass
        if args.stop_stack and stack_started:
            try:
                stack_run("stop", timeout=90)
            except Exception as e:
                log("stack stop failed:", e)


def read_camera_csv(path):
    rows = []
    if not path.exists():
        return rows
    for line in path.read_text(encoding="utf-8").splitlines()[1:]:
        try:
            rows.append(tuple(float(x) for x in line.split(",")))
        except ValueError:
            pass
    return rows


def run_analysis(out, scenario, orig, bevy, server, args):
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    if not args.no_plots:
        try:
            import plots
            plots.make_all(out, scenario, orig, bevy, bevy_label=args.bevy_label)
        except Exception as e:
            log("plots failed:", e)
    try:
        import metrics
        m = metrics.compute(scenario, orig, bevy, bevy_label=args.bevy_label)
        (out / "metrics.json").write_text(json.dumps(m, indent=1), encoding="utf-8")
        log("metrics:", json.dumps(m, indent=1))
    except Exception as e:
        log("metrics failed:", e)
    try:
        import strip
        strip.make(out, scenario, orig, bevy, bevy_label=args.bevy_label)
    except Exception as e:
        log("strip failed:", e)


if __name__ == "__main__":
    main()
