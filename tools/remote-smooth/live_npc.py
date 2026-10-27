"""Sample live X2::NetActorMovementController objects of NPCs (EmptyUnitModel).

One hidden original client (rmsmooth1) at the movement test spot. Scans private memory
for the controller vtable, samples every controller until moving ones are found, then
records one moving NPC's controller fields, engine frame time and drawn entity position.
"""
import ctypes
import ctypes.wintypes as wt
import json
import math
import re
import struct
import subprocess
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
ART = Path(r"C:/Users/stsiakal/AppData/Local/Temp/opencode/npc-mcp")
USER, CHAR = "rmsmooth1", "Rmsmootho"
VT_RVA = 0x9B3308
GAME_GLOBAL_RVA = 0x15CB1D8
CONTROLLER_SIZE = 0x4C0

sys.path.insert(0, str(ROOT / "tools/feel-recorder"))
from run import Driver  # noqa: E402

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.VirtualQueryEx.restype = ctypes.c_size_t


class Mem:
    QUERY = 0x0400
    READ = 0x0010
    COMMIT = 0x1000
    PRIVATE = 0x20000

    class MBI(ctypes.Structure):
        _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                    ("AllocationProtect", wt.DWORD), ("PartitionId", wt.WORD),
                    ("RegionSize", ctypes.c_size_t), ("State", wt.DWORD),
                    ("Protect", wt.DWORD), ("Type", wt.DWORD)]

    def __init__(self, pid):
        self.h = k32.OpenProcess(self.QUERY | self.READ, False, pid)
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
            if not k32.VirtualQueryEx(self.h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
                break
            yield mbi
            step = mbi.RegionSize
            if not step:
                break
            addr = (mbi.BaseAddress or 0) + step

    def find_u32(self, value):
        hits, pattern = [], struct.pack("<I", value)
        for r in self.regions():
            if r.State != self.COMMIT or r.Type != self.PRIVATE:
                continue
            base, size = r.BaseAddress or 0, r.RegionSize
            off = 0
            while off < size:
                n = min(8 << 20, size - off)
                data = self.read(base + off, n)
                if data:
                    pos = 0
                    while True:
                        i = data.find(pattern, pos)
                        if i < 0:
                            break
                        hits.append(base + off + i)
                        pos = i + 4
                off += n
        return hits


def u32b(b, off):
    return struct.unpack_from("<I", b, off)[0]


def f32s(b, off, n):
    return list(struct.unpack_from("<%df" % n, b, off))


def f64s(b, off, n):
    return list(struct.unpack_from("<%dd" % n, b, off))


def fields(mem, addr):
    b = mem.read(addr, CONTROLLER_SIZE)
    if not b or len(b) < CONTROLLER_SIZE:
        return None
    return {
        "addr": addr,
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
    }


def cli(*args, pid):
    p = subprocess.run([T, *map(str, args), "--pid", str(pid)], capture_output=True, text=True, timeout=60)
    return p.stdout.strip() if p.returncode == 0 else None


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    ART.mkdir(parents=True, exist_ok=True)
    d = Driver([T, "mcp", "--dll", DLL, "--client", CLIENT, "--launcher", LAUNCHER,
                "--password-file", PW, "--login", "127.0.0.1:28237", "--user", USER,
                "--artifact-dir", str(ART)])
    pid = None
    try:
        out = d.text("client_launch", fast_start=True, fast_pak=True, hidden=True,
                     user=USER, password_file=PW)
        pid = int(re.search(r"pid (\d+)", out).group(1))
        print("pid", pid, flush=True)
        for _ in range(90):
            if cli("mod", "x2game.dll", pid=pid):
                break
            time.sleep(2)
        stage = ""
        for _ in range(90):
            stage = d.text("client_lua", pid=pid, target="ui",
                           code="return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()").strip()
            if stage == "3":
                break
            time.sleep(2)
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            d.text("client_lua", pid=pid, target="ui", code="X2World:EnterWorld(1,0) return 1")
            for _ in range(20):
                if d.text("client_lua", pid=pid, target="ui",
                          code="return X2LoginCharacter:GetCurrentStage()").strip() == "4":
                    deadline = 0
                    break
                time.sleep(0.5)
        d.text("client_lua", pid=pid, target="ui",
               code="local b=characterSelectWindow.bottomPanel.startButton b:OnClick('LeftButton') return 1")
        for _ in range(120):
            st = d.text("client_lua", pid=pid, target="ui",
                        code='return X2LoginCharacter:GetCurrentStage(), X2Unit and X2Unit:UnitName("player")')
            if st.startswith("0") and "nil" not in st:
                break
            time.sleep(1)
        print("in world", st.replace("\t", " | "), flush=True)
        time.sleep(18)
        base = int(re.search(r"base=0x([0-9a-f]+)",
                             cli("mod", "x2game.dll", pid=pid)).group(1), 16)
        vt = base + VT_RVA
        raw = cli("peek-global", f"0x{base + GAME_GLOBAL_RVA:x}", "ptr", pid=pid)
        game = int(raw.split("value=")[-1].strip(), 0)
        timer = Mem(pid).u32(game + 0x14)
        print("base", hex(base), "game", hex(game), "timer", hex(timer), flush=True)
        mem = Mem(pid)
        hits = mem.find_u32(vt)
        print("controllers", len(hits), flush=True)

        # sample all controllers; keep those whose target moves
        rows = []
        t0 = time.monotonic()
        while time.monotonic() - t0 < 12.0:
            row = {"t": time.monotonic() - t0, "ft": None, "c": []}
            # engine timer vtable slot 0x30: thiscall? it is called with push 0 in code
            out = cli("call", f"0x{mem.u32(mem.u32(timer) + 0x30):x}", "thiscall",
                      f"this=0x{timer:x}", 0, pid=pid)
            if out:
                m = re.search(r"st0_f32=([-0-9.eE]+)", out)
                if m:
                    row["ft"] = float(m.group(1))
            for h in hits:
                f = fields(mem, h)
                if f:
                    row["c"].append(f)
            rows.append(row)
            time.sleep(0.01)
        print("samples", len(rows), "frame time", [r["ft"] for r in rows[:5]], flush=True)

        # rank controllers by target movement
        moves = []
        for i in range(len(hits)):
            ts = [r["c"][i]["target"] for r in rows if len(r["c"]) > i]
            if len(ts) < 2:
                continue
            path = sum(math.dist(ts[k], ts[k - 1]) for k in range(1, len(ts)))
            moves.append((path, i, hits[i], ts[0]))
        moves.sort(reverse=True)
        print("moving controllers", [(round(p, 2), hex(a), [round(x, 1) for x in t]) for p, i, a, t in moves[:5]], flush=True)
        (OUT / "npc_sample.json").write_text(json.dumps(rows))
        (OUT / "npc_rank.json").write_text(json.dumps(
            [{"path": p, "addr": a, "first": t} for p, i, a, t in moves[:10]], indent=1))
        # engine getters: mode (DAT_39ae4e80 vtable+8) and response (DAT_39ae4e8c vtable+0xc)
        for name, rva, slot in [("mode", 0xAF4E80, 0x8), ("response", 0xAF4E8C, 0xC)]:
            ptr = mem.u32(base + rva)
            if ptr:
                out = cli("call", f"0x{mem.u32(ptr + slot):x}", "thiscall",
                          f"this=0x{ptr:x}", pid=pid)
                print(name, ptr, out, flush=True)
    finally:
        if pid:
            try:
                d.text("client_stop", pid=pid)
            except Exception:
                pass
        d.close()


if __name__ == "__main__":
    main()
