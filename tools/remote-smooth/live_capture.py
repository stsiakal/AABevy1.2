"""Capture a remote unit's controller state from the hidden original 1.2 client.

Two hidden clients on the move-golden lane: an observer (movegold3/Movegoldc) and a
mover (movegold2/Movegoldb). The observer's process memory is scanned for
X2::NetActorMovementController objects (vtable x2game+0x9B3308) and sampled while the
mover walks. Outputs JSON snapshots + CSV to docs/research/remote-smoothing/live/.
"""
import ctypes
import ctypes.wintypes as wt
import json
import math
import re
import struct
import sys
import time
from pathlib import Path

ROOT = Path(r"D:/aaemu-client/wt/pr")
OUT = ROOT / "docs/research/remote-smoothing/live"
T = r"D:/aaemu-target/move-golden/debug/aaemu-clientdriver.exe"
DLL = r"D:/aaemu-target/move-golden/aadriver/aadriver.dll"
CLIENT = r"D:/AA1.2/bin32/archeage.exe"
LAUNCHER = r"D:/aaemu-live-move-golden/bin/aaemu-launcher.exe"
PW = r"D:/aaemu-live-move-golden/cap-pw.txt"
ART = Path(r"C:/Users/stsiakal/AppData/Local/Temp/opencode/obs-mcp")
OBS_USER, OBS_CHAR = "rmsmooth1", "Rmsmootho"
MOVE_USER, MOVE_CHAR = "rmsmooth2", "Rmsmoothm"
CONTROLLER_VTABLE_RVA = 0x9B3308
START = (10512.0, 15585.0, 232.0)

sys.path.insert(0, str(ROOT / "tools/feel-recorder"))
from run import Driver  # noqa: E402

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.VirtualQueryEx.restype = ctypes.c_size_t


class Mem:
    PROCESS_QUERY_INFORMATION = 0x0400
    PROCESS_VM_READ = 0x0010
    MEM_COMMIT = 0x1000
    MEM_PRIVATE = 0x20000

    class MBI(ctypes.Structure):
        _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                    ("AllocationProtect", wt.DWORD), ("PartitionId", wt.WORD),
                    ("RegionSize", ctypes.c_size_t), ("State", wt.DWORD),
                    ("Protect", wt.DWORD), ("Type", wt.DWORD)]

    def __init__(self, pid):
        self.h = k32.OpenProcess(self.PROCESS_QUERY_INFORMATION | self.PROCESS_VM_READ, False, pid)
        if not self.h:
            raise OSError(ctypes.get_last_error())

    def read(self, addr, n):
        buf = ctypes.create_string_buffer(n)
        got = ctypes.c_size_t(0)
        if not k32.ReadProcessMemory(self.h, ctypes.c_void_p(addr), buf, n, ctypes.byref(got)):
            return None
        return buf.raw[:got.value]

    def u32(self, addr):
        b = self.read(addr, 4)
        return struct.unpack("<I", b)[0] if b and len(b) == 4 else None

    def regions(self):
        addr = 0
        mbi = self.MBI()
        while addr < 0x7FFF0000:
            got = k32.VirtualQueryEx(self.h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi))
            if not got:
                break
            yield mbi
            addr = (mbi.BaseAddress or 0) + mbi.RegionSize
            if mbi.RegionSize == 0:
                break

    def find_u32(self, value, limit=64):
        hits = []
        pattern = struct.pack("<I", value)
        for r in self.regions():
            if r.State != self.MEM_COMMIT or r.Type != self.MEM_PRIVATE:
                continue
            base = r.BaseAddress or 0
            size = r.RegionSize
            step = 4 << 20
            off = 0
            while off < size:
                n = min(step, size - off)
                data = self.read(base + off, n)
                if data:
                    pos = 0
                    while True:
                        i = data.find(pattern, pos)
                        if i < 0:
                            break
                        hits.append(base + off + i)
                        pos = i + 4
                        if len(hits) >= limit:
                            return hits
                off += n
        return hits


def u32b(b, off=0):
    return struct.unpack_from("<I", b, off)[0]


def f32s(b, off, n):
    return list(struct.unpack_from("<%df" % n, b, off))


def f64s(b, off, n):
    return list(struct.unpack_from("<%dd" % n, b, off))


class Client:
    def __init__(self, d, user, char):
        self.d, self.user, self.char = d, user, char
        self.pid = None

    def lua(self, code, target="system"):
        return self.d.text("client_lua", pid=self.pid, target=target, code=code)

    def ui(self, code):
        return self.lua(code, "ui")

    def key(self, key, ms):
        return self.d.text("client_input", pid=self.pid, key=key, duration_ms=int(ms))

    def release(self):
        return self.d.text("client_input", pid=self.pid, key="release")

    def launch(self):
        out = self.d.text("client_launch", fast_start=True, fast_pak=True, hidden=True,
                          user=self.user, password_file=PW)
        self.pid = int(re.search(r"pid (\d+)", out).group(1))
        print(self.user, "pid", self.pid, flush=True)
        self.wait_stage(3, 260)
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            self.ui("X2World:EnterWorld(1,0) return 1")
            for _ in range(20):
                if self.ui("return X2LoginCharacter:GetCurrentStage()").strip() == "4":
                    deadline = 0
                    break
                time.sleep(0.5)
            if deadline == 0:
                break
        self.ui("local b=characterSelectWindow.bottomPanel.startButton b:OnClick('LeftButton')")
        self.wait_player(260)
        time.sleep(2.0)

    def wait_stage(self, stage, timeout):
        deadline = time.monotonic() + timeout
        last = ""
        while time.monotonic() < deadline:
            try:
                last = self.ui("return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()")
                if last.strip() == str(stage):
                    return
            except Exception as e:
                last = str(e)
            time.sleep(0.5)
        raise RuntimeError(f"{self.user} stage {stage}: {last}")

    def wait_player(self, timeout):
        deadline = time.monotonic() + timeout
        last = ""
        while time.monotonic() < deadline:
            try:
                state = self.ui("return X2LoginCharacter:GetCurrentStage(), X2Unit and X2Unit:UnitName('player')")
                stage, _, name = state.partition("\t")
                if stage == "0" and name not in ("", "nil"):
                    return
                last = state
            except Exception as e:
                last = str(e)
            time.sleep(0.5)
        raise RuntimeError(f"{self.user} player: {last}")

    def place(self, pos):
        self.d.text("client_chat", pid=self.pid, message="/move %.1f %.1f %.1f" % pos)
        time.sleep(2.0)

    def entity_position(self, name):
        code = (
            "local want=string.lower(" + json.dumps(name.lower()) + ") "
            "for _,e in pairs(System.GetEntities() or {}) do "
            " if string.lower(e:GetName())==want then "
            "  local p=e:GetWorldPos() local a=e:GetAngles() local v=e:GetVelocity() "
            "  return string.format('%.5f,%.5f,%.5f,%.5f,%.5f,%.5f,%.5f',p.x,p.y,p.z,a.z,v.x,v.y,v.z) "
            " end end return ''"
        )
        out = self.lua(code)
        if not out:
            return None
        return [float(x) for x in out.split(",")]


def controller_fields(mem, addr):
    b = mem.read(addr, 0x4C0)
    if not b or len(b) < 0x4C0:
        return None
    return {
        "addr": addr,
        "vtable": u32b(b, 0),
        "owner": u32b(b, 4),
        "active": b[0xCD],
        "last_ms": u32b(b, 0x130),
        "prev_pos": f32s(b, 0x4B0, 3),
        "step_vel": f32s(b, 0x4A4, 3),
        "carry": f32s(b, 0x498, 3),
        "target": f64s(b, 0x430, 3),
        "previous_target": f64s(b, 0x418, 3),
        "turn_vel": f64s(b, 0x448, 3),
        "vel": f32s(b, 0x400, 3),
        "stored_vel": f32s(b, 0x370, 3),
        "anim_hold": struct.unpack_from("<f", b, 0x40C)[0],
        "anim_speed": struct.unpack_from("<f", b, 0x410)[0],
        "state": b[0x464],
        "flags": list(b[0x47C:0x4C0]),
    }


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    ART.mkdir(parents=True, exist_ok=True)
    cmd = [T, "mcp", "--dll", DLL, "--client", CLIENT, "--launcher", LAUNCHER,
           "--password-file", PW, "--login", "127.0.0.1:28237", "--user", OBS_USER,
           "--artifact-dir", str(ART)]
    d = Driver(cmd)
    try:
        obs, mov = Client(d, OBS_USER, OBS_CHAR), Client(d, MOVE_USER, MOVE_CHAR)
        obs.launch()
        mov.launch()
        obs.place(START)
        mov.place((START[0] - 8.0, START[1], START[2]))
        for c in (obs, mov):
            try:
                c.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
            except Exception:
                pass
        pos = None
        for _ in range(40):
            pos = mov.entity_position(MOVE_CHAR)
            if pos:
                break
            time.sleep(1)
        if not pos:
            raise RuntimeError("mover entity not found")
        print("mover local pos", pos, flush=True)
        base = None
        out = d.text("client_status", pid=obs.pid)
        print("observer", out[:200], flush=True)
        # module base through the CLI (the MCP holds the client; CLI attaches to the same pipe)
        import subprocess
        mo = subprocess.run([T, "mod", "x2game.dll", "--pid", str(obs.pid)], capture_output=True, text=True).stdout
        base = int(re.search(r"base=0x([0-9a-f]+)", mo).group(1), 16)
        print("observer x2game base", hex(base), flush=True)
        vt = base + CONTROLLER_VTABLE_RVA
        mem = Mem(obs.pid)
        want = (pos[0], pos[1], pos[2])
        deadline = time.monotonic() + 90
        while True:
            hits = mem.find_u32(vt)
            cands = []
            for h in hits:
                f = controller_fields(mem, h)
                if not f:
                    continue
                f["local_dist"] = math.dist(f["prev_pos"], want)
                f["target_dist"] = math.dist(f["target"], [want[0] + 6144.0, want[1] + 11264.0, want[2]])
                cands.append(f)
            cands.sort(key=lambda f: f["local_dist"])
            print("scan", len(hits), "hits, closest", cands[0]["local_dist"] if cands else None, flush=True)
            if cands and cands[0]["local_dist"] < 30.0 and cands[0]["active"]:
                break
            if time.monotonic() > deadline:
                break
            time.sleep(3)
        print(json.dumps([{k: v for k, v in c.items() if k != "flags"} for c in cands[:8]], indent=1), flush=True)
        (OUT / "candidates.json").write_text(json.dumps({"mover_pos": want, "candidates": cands}, indent=1))

        # sample the best candidates while the mover walks forward
        keep = [c["addr"] for c in cands[:3]]
        rows = []
        mov.key("w", 2600)
        t0 = time.monotonic()
        while time.monotonic() - t0 < 4.2:
            ts = time.monotonic() - t0
            mp = mov.entity_position(MOVE_CHAR)
            op = obs.entity_position(MOVE_CHAR)
            entry = {"t": ts, "mover": mp, "observed": op, "c": []}
            for a in keep:
                f = controller_fields(mem, a)
                if f:
                    entry["c"].append(f)
            rows.append(entry)
            time.sleep(0.01)
        mov.release()
        (OUT / "walk.json").write_text(json.dumps(rows, indent=0))
        print("rows", len(rows), "output", OUT, flush=True)
    finally:
        for c in (locals().get("obs"), locals().get("mov")):
            if c is not None and c.pid:
                try:
                    d.text("client_stop", pid=c.pid)
                except Exception:
                    pass
        d.close()


if __name__ == "__main__":
    main()
