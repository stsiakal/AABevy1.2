#!/usr/bin/env python3
"""Record the hidden 1.2 client for the combat side-by-side acceptance run.

It launches (or attaches to) a hidden original client on the trace stack, spawns a
fresh target of one NPC template with the GM command, targets it, runs the timed
script and records:

* the caster/victim actor animation state and clip time from a 16 ms Lua timer,
* the cast bar, health/combat state and action-slot cooldown from a UI Lua timer,
* the caster's unit memory every ~5 ms: GCD, cast slots, in-combat bytes, weapon,
* the scheduler entries of both units from the original's animation controllers,
* every ``PlayAnim`` and hit call through a pass-through hook,
* the lane server log slice with CSStartSkill bodies and S/C replies.

The controller list offsets and the unit fields are the ones the ported
``aa-client`` code and the ``port/combat_anim`` goldens use; the hook plumbing is
imported read-only from the lane's ``rust/tools/capture_combat*`` helpers.

Usage:
    python tools/combat-side/record_original.py \
        --run-dir shots/combat-side/run --login 127.0.0.1:29337 \
        --password-file shots/combat-side/run/pw.txt
"""
from __future__ import annotations

import argparse
import ctypes
import ctypes.wintypes as wt
import json
import os
import re
import struct
import subprocess
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_slots  # noqa: E402

DRIVER = Path(r"D:/aaemu-target/combat-golden/live/aaemu-clientdriver.exe")
DLL = Path(r"D:/aaemu-target/combat-golden/aadriver/aadriver.dll")
CLIENT = Path(r"D:/AA1.2/bin32/archeage.exe")
LAUNCHER = Path(r"D:/aaemu-live-combat-golden/bin/aaemu-launcher.exe")
LANE_TOOLS = Path(r"D:/aaemu-wt/combat-golden/rust/tools")
GAME_MODULE_BASE = 0x38FF0000

# x2game.dll VAs (preferred base) used by the recorder.
VA = {
    "unit_lookup": 0x39360020,
    "play_anim": 0x39157680,
    "hit": 0x39157130,
}
# Unit fields (docs/research/port-combat/structs.md and port/combat_anim).
UNIT_TARGET = 0x1AA0
UNIT_CD = 0x2C48
UNIT_CAST = 0x3AC0
UNIT_STANCE = 0x3BBA
UNIT_WEAPON = 0x3BA8
UNIT_WEAPON_SLOTS = 0x3BB0
UNIT_VISUAL_BYTE = 0x3C30
UNIT_COMBAT_DEADLINE = 0x5FC0
UNIT_COMBAT_IN = 0x5FC4
UNIT_COMBAT_LATCH_A = 0x5FC5
UNIT_COMBAT_LATCH_B = 0x5FC6

KIND = {0: "none", 1: "item_type", 2: "spell", 3: "macro", 4: "item_id", 5: "ride_pet_spell"}
NAME = None


def log(*a: object) -> None:
    print("original:", *a, flush=True)


class Mcp:
    def __init__(self, password_file: Path, login: str, user: str):
        self.p = subprocess.Popen(
            [str(DRIVER), "mcp", "--dll", str(DLL), "--client", str(CLIENT),
             "--launcher", str(LAUNCHER), "--password-file", str(password_file),
             "--login", login, "--user", user, "--artifact-dir", str(RUN / "mcp")],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, encoding="utf-8")
        self.seq = 0
        self.call("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                 "clientInfo": {"name": "combat-side", "version": "1"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def call(self, method, params=None):
        self.seq += 1
        req = {"jsonrpc": "2.0", "id": self.seq, "method": method}
        if params is not None:
            req["params"] = params
        self.p.stdin.write(json.dumps(req) + "\n")
        self.p.stdin.flush()
        line = self.p.stdout.readline()
        if not line:
            raise RuntimeError("MCP closed")
        r = json.loads(line)
        if r.get("id") != self.seq:
            raise RuntimeError(f"MCP id mismatch: {r}")
        if "error" in r:
            raise RuntimeError(str(r["error"]))
        return r["result"]

    def tool(self, name, **args):
        r = self.call("tools/call", {"name": name, "arguments": args})
        if r.get("isError"):
            raise RuntimeError("\n".join(c.get("text", "") for c in r.get("content", [])))
        return r

    def text(self, name, **args):
        return "\n".join(c["text"] for c in self.tool(name, **args)["content"]
                         if c["type"] == "text")

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=10)
        except Exception:
            self.p.kill()


class Mem:
    PROCESS_QUERY_INFORMATION = 0x0400
    PROCESS_VM_READ = 0x0010

    def __init__(self, pid: int):
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.handle = self.kernel.OpenProcess(
            self.PROCESS_QUERY_INFORMATION | self.PROCESS_VM_READ, False, pid)
        if not self.handle:
            raise OSError(ctypes.get_last_error())

    def read(self, addr: int, size: int) -> bytes | None:
        buf = ctypes.create_string_buffer(size)
        got = ctypes.c_size_t()
        if not self.kernel.ReadProcessMemory(
                self.handle, ctypes.c_void_p(addr), buf, size, ctypes.byref(got)):
            return None
        return buf.raw[:got.value]

    def u32(self, addr: int) -> int:
        b = self.read(addr, 4)
        return struct.unpack("<I", b)[0] if b and len(b) == 4 else 0

    def u8(self, addr: int) -> int:
        b = self.read(addr, 1)
        return b[0] if b else 0

    def f32(self, addr: int) -> float:
        b = self.read(addr, 4)
        return struct.unpack("<f", b)[0] if b and len(b) == 4 else 0.0

    def u32s(self, addr: int, n: int) -> list[int]:
        b = self.read(addr, 4 * n)
        return list(struct.unpack("<%dI" % n, b)) if b and len(b) == 4 * n else [0] * n


def s32(v: int) -> int:
    return v - 0x100000000 if v & 0x80000000 else v


def f32_of_bits(v: int) -> float:
    return struct.unpack("<f", struct.pack("<I", v & 0xFFFFFFFF))[0]


class ControllerSampler(threading.Thread):
    def __init__(self, mem: Mem, caster: int, victim: int, state: dict | None = None):
        super().__init__(daemon=True)
        self.mem = mem
        self.caster = caster
        self.victim = victim
        self.state = state or {}
        self.stop_event = threading.Event()
        self.rows: list[str] = []
        self.hook = None
        self.last_seq = 0
        self.play_rows: list[dict] = []

    def read_entry(self, ctrl: int, off: int):
        head_ptr = self.mem.u32(ctrl + off)
        if not head_ptr:
            return None
        first = self.mem.u32(head_ptr)
        if not first or first == head_ptr:
            return None
        data = self.mem.read(first + 8, 0x44)
        if not data or len(data) < 0x44:
            return None
        words = struct.unpack("<17I", data)
        start = struct.unpack_from("<f", data, 0x28)[0]
        return {
            "id": words[0],
            "weapon": s32(words[1]),
            "anim_id": s32(words[2]),
            "data": words[8],
            "start": start,
            "duration": struct.unpack_from("<f", data, 0x2C)[0],
            "fire": struct.unpack_from("<f", data, 0x30)[0],
        }

    def entries(self, ctrl: int):
        out = []
        for off, name in ((0x24, "A"), (0x30, "B")):
            entry = self.read_entry(ctrl, off)
            if entry:
                last = self.mem.f32(ctrl + 0x70)
                entry["list"] = name
                entry["last_time"] = last
                entry["clip_time"] = max(0.0, last - entry["start"]) if entry["start"] > 0 else 0.0
            out.append(entry)
        return out

    def hook_records(self):
        if self.hook is None:
            return []
        head = self.mem.u32(self.hook.head)
        out = []
        for i in range(min(head, 128)):
            raw = self.mem.read(self.hook.rec + i * self.hook.record_size,
                                self.hook.record_size)
            if not raw or len(raw) < 64:
                continue
            words = struct.unpack("<16I", raw[:64])
            out.append({"seq": words[0], "ret": words[1], "args": list(words[2:10]),
                        "ecx": words[10], "edx": words[11], "eax": words[12],
                        "done": words[13], "out1": words[14], "out2": words[15]})
        return out

    def poll_hook(self):
        if self.hook is None:
            return
        recs = self.hook_records()
        new = [r for r in recs if r["seq"] > self.last_seq]
        if not new:
            return
        self.last_seq = max(r["seq"] for r in new)
        new.sort(key=lambda r: r["seq"])
        for r in new:
            now = time.time() * 1000.0
            ecx = r["ecx"]
            args = r["args"]
            row = {
                "wall_ms": now,
                "seq": r["seq"],
                "ctrl": ecx,
                "anim_id": s32(args[1]),
                "weapon": s32(args[2]),
                "data": args[3],
                "flags": args[4],
                "cast": f32_of_bits(args[5]),
                "chan": f32_of_bits(args[6]),
            }
            self.play_rows.append(row)
            if ecx and ecx not in (0, 0xFFFFFFFF):
                if args[3] == self.victim and self.victim:
                    self.state["victim_ctrl"] = ecx
                if not self.state.get("caster_ctrl") and args[3] != self.victim:
                    # The first recorded scheduler call is the caster's own skill.
                    self.state["caster_ctrl"] = ecx
                if (self.state.get("caster_ctrl") and ecx != self.state["caster_ctrl"]
                        and s32(args[1]) in (14, 17, 417, 418)):
                    # A basic hit reaction on the other unit's controller.
                    self.state["victim_ctrl"] = ecx
                model = self.mem.u32(ecx + 4)
                uid = 0
                if model:
                    for off in (0x1FC, 0x8, 0x0):
                        uid = self.mem.u32(model + off)
                        if uid in (self.caster, self.victim):
                            break
                if uid == self.caster:
                    self.state["caster_ctrl"] = ecx
                elif uid == self.victim:
                    self.state["victim_ctrl"] = ecx

    def run(self):
        while not self.stop_event.is_set():
            try:
                self.poll_hook()
                now = time.time() * 1000.0
                u = self.state.get("unit")
                ctrl_a = self.state.get("caster_ctrl")
                ctrl_v = self.state.get("victim_ctrl")
                gcd = (0, 0)
                cast = [0, 0, 0, 0]
                stance = (0, 0, 0, 0, 0)
                weapon = (0, (0,) * 6, 0)
                if u:
                    gcd = tuple(self.mem.u32s(u + UNIT_CD + 8, 2))
                    cast = self.mem.u32s(u + UNIT_CAST + 4, 4)
                    stance = (self.mem.u8(u + UNIT_STANCE),
                              self.mem.u32(u + UNIT_COMBAT_DEADLINE),
                              self.mem.u8(u + UNIT_COMBAT_IN),
                              self.mem.u8(u + UNIT_COMBAT_LATCH_A),
                              self.mem.u8(u + UNIT_COMBAT_LATCH_B))
                    weapon = (self.mem.u32(u + UNIT_WEAPON),
                              tuple(self.mem.u32s(u + UNIT_WEAPON_SLOTS, 6)),
                              self.mem.u8(u + UNIT_VISUAL_BYTE))
                ca = self.entries(ctrl_a) if ctrl_a else [None, None]
                cv = self.entries(ctrl_v) if ctrl_v else [None, None]
                row = [
                    "%.1f" % now,
                    str(self.state.get("caster_ctrl", 0)),
                    str(self.state.get("victim_ctrl", 0)),
                    str(gcd[0]), str(gcd[1]),
                    *map(str, cast),
                    str(stance[0]), str(stance[1]), str(stance[2]), str(stance[3]), str(stance[4]),
                    str(weapon[0]), *map(str, weapon[1]), str(weapon[2]),
                ]
                for entry in (ca[0], ca[1], cv[0], cv[1]):
                    if entry:
                        row += [str(entry["anim_id"]), "%.4f" % entry["start"],
                                "%.4f" % entry["duration"], "%.4f" % entry["clip_time"],
                                str(entry["weapon"]), str(entry["data"])]
                    else:
                        row += ["", "", "", "", "", ""]
                self.rows.append(",".join(row))
            except Exception:
                pass
            self.stop_event.wait(0.005)

    def stop(self):
        self.stop_event.set()
        self.join(timeout=5)


def lua_timer(sample_code: str, interval: int = 16) -> str:
    return r"""
_G.CS_RUN = false
_G.CS_STOP = false
_G.CS_ROWS = {}
local function sample()
%s
end
local function tick()
  if _G.CS_STOP then return end
  if _G.CS_RUN then
    local ok, row = pcall(sample)
    if ok and type(row) == 'string' and row ~= 'noent' then
      _G.CS_ROWS[#_G.CS_ROWS + 1] = row
    end
  end
  Script.SetTimer(%d, tick)
end
Script.SetTimer(%d, tick)
return 'installed'
""" % (sample_code, interval, interval)


SYSTEM_SAMPLE = r"""
local pe, te, best
for _,x in pairs(System.GetEntities() or {}) do
  local n=x:GetName()
  if n == CHAR_NAME then pe=x end
  if n == TARGET_NAME then
    local ok,p=pcall(function() return x:GetWorldPos() end)
    if ok and p and pe then
      local pp=pe:GetWorldPos()
      local dx,dy,dz=p.x-pp.x,p.y-pp.y,p.z-pp.z
      local d=dx*dx+dy*dy+dz*dz
      if best == nil or d < best then best=d; te=x end
    end
  end
end
if pe == nil then return 'noent' end
local function st(x)
  if x == nil or x.actor == nil then return 'nil','' end
  local s='?'; pcall(function() s=x.actor:GetCurrentAnimationState() end)
  local t=''; pcall(function() t=string.format('%.4f', x.actor:GetAnimationTime(0,0)) end)
  return s,t
end
local ps,pt = st(pe)
local vs,vt = st(te)
local function vec(p)
  if type(p) ~= 'table' then return 'nil' end
  return string.format('%.4f,%.4f,%.4f', p.x or 0, p.y or 0, p.z or 0)
end
return table.concat({
  tostring(System.GetFrameID()), string.format('%.6f', System.GetCurrTime()),
  ps, pt, vs, vt, vec(pe:GetWorldPos()),
}, '|')
"""

UI_SAMPLE = r"""
local sec = os.time()
if _G.CS_UI_SEC ~= sec then _G.CS_UI_SEC = sec _G.CS_UI_N = 0 else _G.CS_UI_N = (_G.CS_UI_N or 0) + 1 end
local name,total,cur,desc = X2Unit:UnitCastingInfo('player')
local tn,tt,tc,td = X2Unit:UnitCastingInfo('target')
local cd = -1
if X2Action and X2Action.GetActionCooldown then cd = X2Action:GetActionCooldown(1) end
return table.concat({
  tostring(sec * 1000 + _G.CS_UI_N * 16),
  tostring(name or ''), tostring(total or ''), tostring(cur or ''), tostring(desc or ''),
  tostring(tn or ''), tostring(tt or ''), tostring(tc or ''), tostring(td or ''),
  tostring(cd),
  tostring(X2Unit:UnitHealth('player')), tostring(X2Unit:UnitMana('player')),
  tostring(X2Unit:UnitHealth('target')), tostring(X2Unit:UnitMaxHealth('target')),
  tostring(X2Unit:UnitIsDead('player')), tostring(X2Unit:UnitCombatState('player')),
}, '|')
"""

CHAT_HOOK = r"""
if not _G.__combat_side then _G.__combat_side={chat={}} end
local o=_G.__combat_side
if not o.hooked then
 if type(X2Chat)~='table' or type(X2Chat.DispatchChatMessage)~='function' then return 'unavailable' end
 local orig=X2Chat.DispatchChatMessage
 X2Chat.DispatchChatMessage=function(self,...)
  if #o.chat<8192 then
   local n=select('#',...)
   local parts={}
   for i=1,n do parts[i]=tostring((select(i,...))) end
   o.chat[#o.chat+1]=table.concat(parts,'\t')
  end
  return orig(self,...)
 end
 o.hooked=true
end
return 'hooked'
"""


def parse_chat(mcp: Mcp, pid: int) -> str:
    return mcp.text("client_lua", pid=pid, target="ui",
                    code="local o=assert(_G.__combat_side) return table.concat(o.chat,'\\n')")


def send_chat(mcp: Mcp, pid: int, msg: str) -> None:
    mcp.text("client_lua", pid=pid, target="ui",
             code="local o=assert(_G.__combat_side) o.chat={} X2Chat:SendChatMessage(CHAT_SAY,%s,'') return 'sent'" %
             json.dumps(msg))


def enter_world(mcp: Mcp, password_file: Path, user: str, timeout: float = 240.0):
    clients = json.loads(mcp.text("client_list"))
    mine = [c for c in clients if c.get("launched_by_session")]
    if mine:
        pid = mine[-1]["pid"]
    else:
        log("launch", user)
        mcp.text("client_launch", fast_start=True, fast_pak=True, hidden=True,
                 user=user, password_file=str(password_file))
        pid = None
        deadline = time.time() + 60
        while time.time() < deadline:
            clients = json.loads(mcp.text("client_list"))
            mine = [c for c in clients if c.get("launched_by_session")]
            if mine:
                pid = mine[-1]["pid"]
                break
            time.sleep(0.5)
    if pid is None:
        raise RuntimeError("client did not start")
    log("pid", pid)
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        try:
            last = mcp.text("client_lua", pid=pid, target="ui",
                            code="return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()")
            if last.strip() == "3":
                break
        except Exception as e:
            last = str(e)
        time.sleep(0.5)
    if last.strip() != "3":
        raise RuntimeError("login stage 3 not reached: " + last)
    mcp.text("client_lua", pid=pid, target="ui", code="X2World:EnterWorld(1,0) return 1")
    while time.time() < deadline:
        try:
            last = mcp.text("client_lua", pid=pid, target="ui",
                            code="return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()")
            if last.strip() == "4":
                break
        except Exception as e:
            last = str(e)
        time.sleep(0.5)
    if last.strip() != "4":
        raise RuntimeError("character select not reached: " + last)
    mcp.text("client_lua", pid=pid, target="ui",
             code="local b=characterSelectWindow.bottomPanel.startButton b:OnClick('LeftButton') return 1")
    while time.time() < deadline:
        try:
            state = mcp.text("client_lua", pid=pid, target="ui",
                             code="return X2LoginCharacter:GetCurrentStage(), X2Unit and X2Unit:UnitName('player')")
            stage, _, name = state.partition("\t")
            if stage.strip() == "0" and name.strip() not in ("", "nil"):
                log("player", name)
                # The HUD action bar and its key bindings come up after the world;
                # wait for slot 1's key label before the recorded script starts.
                ready = False
                for _ in range(40):
                    try:
                        label = mcp.text(
                            "client_lua", pid=pid, target="ui",
                            code="local b=actionBar_renewal and actionBar_renewal[1] "
                                 "and actionBar_renewal[1].slot[1] "
                                 "return tostring(b and b.keybindingLabel and b.keybindingLabel:GetText())")
                        if label.strip() not in ("", "nil"):
                            ready = True
                            break
                    except Exception:
                        pass
                    time.sleep(0.5)
                log("action bar ready", ready)
                time.sleep(1.5)
                return pid
        except Exception as e:
            last = str(e)
        time.sleep(0.5)
    raise RuntimeError("player did not appear: " + last)


def main() -> None:
    global RUN, NAME
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--login", default="127.0.0.1:29337")
    parser.add_argument("--slot", default=os.environ.get("COMBAT_SIDE_SLOT"),
                        help="worker slot; sets --user/--character to the slot's original account")
    parser.add_argument("--user", default="clienttest")
    parser.add_argument("--character", default="Goldtest")
    parser.add_argument("--npc", type=int, default=3968)
    parser.add_argument("--npc-name", default="fox_test")
    parser.add_argument("--start", default="15577.7,15382.3,132.4")
    parser.add_argument("--spawn", action="store_true",
                        help="despawn/spawn a fresh NPC instead of using a static one")
    parser.add_argument("--password-file", type=Path, required=True)
    parser.add_argument("--server-log", type=Path, required=True)
    parser.add_argument("--pid", type=int, default=None)
    parser.add_argument("--keep-client", action="store_true")
    args = parser.parse_args()
    if args.slot:
        account = live_slots.resolve(args.slot, "combat-golden", "a")
        args.user = account.user
        args.character = account.character
        offset = live_slots.slot_index(args.slot) * 8.0
        x, y, z = (float(v) for v in args.start.split(","))
        args.start = f"{x + offset:.1f},{y + offset * 0.5:.1f},{z:.1f}"

    RUN = args.run_dir
    RUN.mkdir(parents=True, exist_ok=True)
    NAME = args.character
    mcp = Mcp(args.password_file, args.login, args.user)
    pid = None
    sampler = None
    hook = None
    api = None
    try:
        if args.pid:
            pid = args.pid
            log("attach pid", pid)
        else:
            pid = enter_world(mcp, args.password_file, args.user)
        # A live GM move to the flat training pond: the character row is restored
        # afterwards, and this avoids the slope the first fox attempt stood on.
        sx, sy, sz = (float(v) for v in args.start.split(","))
        log("move", args.start)
        mcp.text("client_chat", pid=pid, message="/move %.3f %.3f %.3f" % (sx, sy, sz))
        time.sleep(2.5)

        # read-only lane helpers (hook + memory access)
        sys.path.insert(0, str(LANE_TOOLS))
        import capture_combat_goldens as stage_a  # noqa: E402
        import capture_combat_live as live_mod  # noqa: E402

        driver = stage_a.Driver(str(DRIVER), pid)
        live = live_mod.Live(driver, pid)
        api = (driver, live)
        log("local unit", live.actor_id, hex(live.actor))

        log("install chat hook", mcp.text("client_lua", pid=pid, target="ui", code=CHAT_HOOK))
        if args.spawn:
            send_chat(mcp, pid, f"/despawn npc {args.npc} 30")
            time.sleep(1.0)
            send_chat(mcp, pid, f"/spawn npc {args.npc}")
            time.sleep(2.0)
        send_chat(mcp, pid, f"/findobject npc {args.npc} 2")
        time.sleep(1.5)
        chat = parse_chat(mcp, pid)
        (RUN / "setup_chat.txt").write_text(chat, encoding="utf-8")
        ids = re.findall(r"BcId:\s*(\d+)", chat)
        if not ids:
            raise RuntimeError("spawned NPC not found in chat:\n" + chat)
        victim_id = int(ids[0])
        target = mcp.text("client_target", pid=pid, unit_id=format(victim_id, "X"),
                          expected_name=args.npc_name)
        log("target", target)
        target_state = json.loads(mcp.text("client_unit_state", pid=pid, target="target"))
        if not target_state.get("present"):
            raise RuntimeError("target is not present")
        distance = float(target_state.get("distance", 999))
        if distance > 4.0:
            pos = re.search(r"X:([-0-9,.]+)\s+Y:([-0-9,.]+)\s+Z:([-0-9,.]+)", chat)
            if not pos:
                raise RuntimeError("target position missing from findobject reply")
            tx, ty, tz = (float(v.replace(",", "")) for v in pos.groups())
            mcp.text("client_chat", pid=pid,
                     message="/move %.3f %.3f %.3f" % (tx, ty - 2.0, tz))
            time.sleep(1.0)
            send_chat(mcp, pid, f"/findobject npc {args.npc} 2")
            time.sleep(1.0)
            chat = parse_chat(mcp, pid)
            (RUN / "setup_chat.txt").write_text(chat, encoding="utf-8")
            ids = re.findall(r"BcId:\s*(\d+)", chat)
            if not ids:
                raise RuntimeError("target lost after move:\n" + chat)
            victim_id = int(ids[0])
            target = mcp.text("client_target", pid=pid, unit_id=format(victim_id, "X"),
                              expected_name=args.npc_name)
            log("target after move", target)
            target_state = json.loads(mcp.text("client_unit_state", pid=pid, target="target"))
            distance = float(target_state.get("distance", 999))
        if distance > 4.0:
            raise RuntimeError("target is out of melee range: %s" % distance)
        player = json.loads(mcp.text("client_unit_state", pid=pid, target="player"))
        player_id = int(player["unit_id"], 16)
        log("player id", hex(player_id), "victim", hex(victim_id), "distance", target_state.get("distance"))

        # The first PlayAnim of the recorded cast identifies the caster controller; a
        # victim reaction identifies the other one.
        hook = live_mod.PassThroughHook(live, VA["play_anim"])
        hook.install()
        mem = Mem(pid)
        sampler_state = {"unit": live.actor, "caster_ctrl": None, "victim_ctrl": None}
        sampler = ControllerSampler(mem, player_id, victim_id, sampler_state)
        sampler.hook = hook
        sampler.last_seq = 0

        sys_code = (SYSTEM_SAMPLE.replace("CHAR_NAME", json.dumps(args.character))
                    .replace("TARGET_NAME", json.dumps(args.npc_name)))
        log("install system timer", mcp.text("client_lua", pid=pid, target="system",
                                             code=lua_timer(sys_code)))
        log("install ui timer", mcp.text("client_lua", pid=pid, target="ui",
                                         code=lua_timer(UI_SAMPLE)))
        mcp.text("client_lua", pid=pid, target="system", code="_G.CS_RUN=true return 1")
        mcp.text("client_lua", pid=pid, target="ui", code="_G.CS_UI_RUN=true return 1")

        server_offset = args.server_log.stat().st_size if args.server_log.exists() else 0
        sampler.start()
        wall0 = time.time()
        script = [
            (1.0, "key", "1"),
            (4.5, "key", "2"),
            (7.5, "key", "3"),
        ]
        inputs = []
        for at, kind, value in script:
            delay = at - (time.time() - wall0)
            if delay > 0:
                time.sleep(delay)
            before = time.time() * 1000.0
            if kind == "key":
                result = mcp.text("client_input", pid=pid, key=value,
                                  duration_ms=200 if value == "s" else 80)
            else:
                result = mcp.text("client_chat", pid=pid, message="/move " + value)
            after = time.time() * 1000.0
            inputs.append({"at": at, "kind": kind, "value": value,
                           "before_ms": before, "after_ms": after, "result": result})
            log(kind, value, "at %.2f" % (time.time() - wall0), result)
        end = 40.0 - (time.time() - wall0)
        if end > 0:
            time.sleep(end)

        mcp.text("client_lua", pid=pid, target="system", code="_G.CS_RUN=false _G.CS_STOP=true return 1")
        mcp.text("client_lua", pid=pid, target="ui", code="_G.CS_UI_RUN=false _G.CS_UI_STOP=true return 1")
        time.sleep(0.3)
        sampler.stop()
        server_end = args.server_log.stat().st_size if args.server_log.exists() else 0
        if server_end > server_offset:
            with args.server_log.open("rb") as f:
                f.seek(server_offset)
                (RUN / "server_slice.log").write_bytes(f.read(server_end - server_offset))

        def fetch(table, target):
            total = int(mcp.text("client_lua", pid=pid, target=target,
                                 code="return #%s" % table).strip() or 0)
            rows = []
            for start in range(1, total + 1, 400):
                end_i = min(total, start + 399)
                text = mcp.text("client_lua", pid=pid, target=target,
                                code="return table.concat(%s, '\\n', %d, %d)" % (table, start, end_i))
                if text:
                    rows.extend(text.splitlines())
            return rows

        lua_rows = fetch("_G.CS_ROWS", "system")
        ui_rows = fetch("_G.CS_ROWS", "ui")
        (RUN / "lua_rows.txt").write_text("\n".join(lua_rows), encoding="utf-8")
        (RUN / "ui_rows.txt").write_text("\n".join(ui_rows), encoding="utf-8")
        (RUN / "controller_samples.csv").write_text(
            "wall_ms,caster_ctrl,victim_ctrl,gcd_remaining,gcd_total,"
            "cast,cast_delay,channel,channel_delay,stance_perm,combat_deadline,combat_in,"
            "latch_a,latch_b,weapon_active,slot0,slot1,slot2,slot3,slot4,slot5,"
            "visual_byte,"
            "caster_A_anim,caster_A_start,caster_A_duration,caster_A_clip,"
            "caster_A_weapon,caster_A_data,"
            "caster_B_anim,caster_B_start,caster_B_duration,caster_B_clip,"
            "caster_B_weapon,caster_B_data,"
            "victim_A_anim,victim_A_start,victim_A_duration,victim_A_clip,"
            "victim_A_weapon,victim_A_data,"
            "victim_B_anim,victim_B_start,victim_B_duration,victim_B_clip,"
            "victim_B_weapon,victim_B_data\n"
            + "\n".join(sampler.rows) + "\n", encoding="utf-8")
        import csv as _csv
        with (RUN / "playanim.csv").open("w", newline="", encoding="utf-8") as f:
            w = _csv.DictWriter(f, fieldnames=["wall_ms", "seq", "ctrl", "anim_id", "weapon",
                                               "data", "flags", "cast", "chan"])
            w.writeheader()
            w.writerows(sampler.play_rows)
        meta = {
            "pid": pid,
            "character": args.character,
            "player_id": player_id,
            "victim_id": victim_id,
            "victim_name": args.npc_name,
            "npc_template": args.npc,
            "target_distance": target_state.get("distance"),
            "caster_ctrl": sampler_state.get("caster_ctrl"),
            "victim_ctrl": sampler_state.get("victim_ctrl"),
            "wall_start_ms": wall0 * 1000.0,
            "inputs": inputs,
            "lua_rows": len(lua_rows),
            "ui_rows": len(ui_rows),
            "controller_samples": len(sampler.rows),
            "playanim": len(sampler.play_rows),
        }
        (RUN / "meta.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
        log("saved", len(lua_rows), "lua rows,", len(ui_rows), "ui rows,",
            len(sampler.rows), "memory rows,", len(sampler.play_rows), "PlayAnim calls")
    finally:
        if sampler is not None:
            try:
                sampler.stop()
            except Exception:
                pass
        if hook is not None:
            try:
                hook.restore()
                hook.free()
            except Exception:
                pass
        if api is not None:
            try:
                api[0].free_all()
            except Exception:
                pass
        if pid and not args.keep_client:
            try:
                mcp.text("client_stop", pid=pid)
            except Exception:
                pass
        mcp.close()


if __name__ == "__main__":
    main()
