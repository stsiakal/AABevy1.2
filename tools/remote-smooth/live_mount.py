"""Two hidden clients: the observer records a remote mount's controller while the
mover rides it (move-golden lane, rmsmooth1/rmsmooth2)."""
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
ART = Path(r"C:/Users/stsiakal/AppData/Local/Temp/opencode/mount-mcp")
OBS_USER, OBS_CHAR = "rmsmooth1", "Rmsmootho"
MOVE_USER, MOVE_CHAR = "rmsmooth2", "Rmsmoothm"
START = (10512.0, 15585.0, 232.0)
OFFSET = (6144.0, 11264.0)
VT_RVA = 0x9B3308
GAME_RVA = 0x15CB1D8
SIZE = 0x4C0

sys.path.insert(0, str(ROOT / "tools/feel-recorder"))
from run import Driver  # noqa: E402
from live_npc import Mem, fields, cli  # noqa: E402


class Client:
    def __init__(self, d, user, char):
        self.d, self.user, self.char, self.pid = d, user, char, None

    def lua(self, code, target="system"):
        return self.d.text("client_lua", pid=self.pid, target=target, code=code)

    def ui(self, code):
        return self.lua(code, "ui")

    def chat(self, msg):
        return self.d.text("client_chat", pid=self.pid, message=msg)

    def key(self, key, ms):
        return self.d.text("client_input", pid=self.pid, key=key, duration_ms=int(ms))

    def release(self):
        return self.d.text("client_input", pid=self.pid, key="release")

    def launch(self):
        out = self.d.text("client_launch", fast_start=True, fast_pak=True, hidden=True,
                          user=self.user, password_file=PW)
        self.pid = int(re.search(r"pid (\d+)", out).group(1))
        print(self.user, self.pid, flush=True)
        for _ in range(120):
            if cli("mod", "x2game.dll", pid=self.pid):
                break
            time.sleep(2)
        for _ in range(120):
            try:
                if self.ui("return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()").strip() == "3":
                    break
            except Exception:
                pass
            time.sleep(1)
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            self.ui("X2World:EnterWorld(1,0) return 1")
            for _ in range(20):
                if self.ui("return X2LoginCharacter:GetCurrentStage()").strip() == "4":
                    deadline = 0
                    break
                time.sleep(0.5)
        self.ui("local b=characterSelectWindow.bottomPanel.startButton b:OnClick('LeftButton')")
        for _ in range(120):
            try:
                st = self.ui('return X2LoginCharacter:GetCurrentStage(), X2Unit and X2Unit:UnitName("player")')
                if st.startswith("0") and "nil" not in st:
                    break
            except Exception:
                pass
            time.sleep(1)
        time.sleep(2)
        try:
            self.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass

    def entity(self, needle):
        code = (
            "local want=string.lower(" + json.dumps(needle.lower()) + ") local best\n"
            "for _,e in pairs(System.GetEntities() or {}) do "
            " local n=string.lower(e:GetName()) "
            " if string.find(n,want,1,true) then "
            "  local p=e:GetWorldPos() local a=e:GetAngles() local v=e:GetVelocity() "
            "  return string.format('%.5f,%.5f,%.5f,%.5f,%.5f,%.5f,%.5f,%s',p.x,p.y,p.z,a.z,v.x,v.y,v.z,e:GetName()) "
            " end end return ''"
        )
        out = self.lua(code)
        if not out:
            return None
        parts = out.split(",")
        return [float(x) for x in parts[:7]] + [parts[7] if len(parts) > 7 else ""]


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    d = Driver([T, "mcp", "--dll", DLL, "--client", CLIENT, "--launcher", LAUNCHER,
                "--password-file", PW, "--login", "127.0.0.1:28237", "--user", OBS_USER,
                "--artifact-dir", str(ART)])
    obs, mov = Client(d, OBS_USER, OBS_CHAR), Client(d, MOVE_USER, MOVE_CHAR)
    try:
        obs.launch()
        mov.launch()
        obs.chat("/move %.1f %.1f %.1f" % START)
        mov.chat("/move %.1f %.1f %.1f" % (START[0] - 8.0, START[1], START[2]))
        time.sleep(2)
        mov.chat("/item add self 8159")
        time.sleep(1.5)
        for _ in range(20):
            if mov.ui("return tostring(X2Mate:IsPlayerPetExists())").strip() == "true":
                break
            slot = None
            try:
                inv = json.loads(mov.d.text("client_inventory", pid=mov.pid))
                slot = next((int(k) for k, v in inv["slots"].items()
                             if v.get("item_template_id") == 8159 and not v.get("dead")), None)
            except Exception:
                pass
            if slot is not None:
                mov.ui("X2Bag:HandleUse(%d) return 1" % slot)
            else:
                mov.ui("return X2Bag:UseItemByType(8159)")
            time.sleep(1.5)
        print("pet", mov.ui("return tostring(X2Mate:IsPlayerPetExists())"), flush=True)
        for _ in range(30):
            mov.ui("X2Mate:MountMate() return 1")
            if mov.ui("return tostring(X2Mate:IsPlayerMounting())").strip() == "true":
                break
            time.sleep(0.5)
        time.sleep(2)
        print("mounted", mov.ui("return tostring(X2Mate:IsPlayerMounting())"), flush=True)

        base = int(re.search(r"base=0x([0-9a-f]+)", cli("mod", "x2game.dll", pid=obs.pid)).group(1), 16)
        raw = cli("peek-global", f"0x{base + GAME_RVA:x}", "ptr", pid=obs.pid)
        game = int(raw.split("value=")[-1].strip(), 0)
        mem = Mem(obs.pid)
        timer = mem.u32(game + 0x14)
        print("base", hex(base), "game", hex(game), "timer", hex(timer), flush=True)
        for name, rva, slot in [("mode", 0xAF4E80, 0x8), ("response", 0xAF4E8C, 0xC)]:
            ptr = mem.u32(base + rva)
            print(name, "ptr", hex(ptr) if ptr else None, flush=True)
            if ptr:
                print(cli("call", f"0x{mem.u32(ptr + slot):x}", "thiscall", f"this=0x{ptr:x}", pid=obs.pid), flush=True)

        horse = obs.entity("horse")
        print("horse", horse, flush=True)
        if not horse:
            raise RuntimeError("observer sees no horse")
        local = horse[:3]
        want = (local[0] + OFFSET[0], local[1] + OFFSET[1], local[2])
        hits = mem.find_u32(base + VT_RVA)
        cands = []
        for h in hits:
            f = fields(mem, h)
            if f:
                f["dist"] = math.dist(f["target"], want)
                f["local_dist"] = math.dist(f["prev_pos"], local)
                cands.append(f)
        cands.sort(key=lambda f: min(f["dist"], f["local_dist"]))
        print("controllers", len(hits), "best", [(hex(c["addr"]), round(c["dist"], 2), round(c["local_dist"], 2)) for c in cands[:4]], flush=True)
        keep = [c["addr"] for c in cands[:3]]
        rows = []
        mov.key("w", 5200)
        t0 = time.monotonic()
        while time.monotonic() - t0 < 6.5:
            row = {"t": time.monotonic() - t0, "ft": None, "horse": obs.entity("horse"),
                   "player": obs.entity(MOVE_CHAR), "c": []}
            out = cli("call", f"0x{mem.u32(mem.u32(timer) + 0x30):x}", "thiscall",
                      f"this=0x{timer:x}", 0, pid=obs.pid)
            if out:
                m = re.search(r"st0_f32=([-0-9.eE]+)", out)
                if m:
                    row["ft"] = float(m.group(1))
            for a in keep:
                f = fields(mem, a)
                if f:
                    row["c"].append(f)
            rows.append(row)
            time.sleep(0.005)
        mov.release()
        (OUT / "mount_walk.json").write_text(json.dumps(rows))
        print("rows", len(rows), "ft", [r["ft"] for r in rows[:6]], flush=True)
        for a in keep:
            seq = [(r["t"], next((c for c in r["c"] if c["addr"] == a), None)) for r in rows]
            seq = [(t, c) for t, c in seq if c]
            if len(seq) > 2:
                path = sum(math.dist(seq[i][1]["target"], seq[i - 1][1]["target"]) for i in range(1, len(seq)))
                print(hex(a), "target path", round(path, 2), "n", len(seq), flush=True)
        return keep
    finally:
        for c in (obs, mov):
            if c.pid:
                try:
                    d.text("client_stop", pid=c.pid)
                except Exception:
                    pass
        d.close()


if __name__ == "__main__":
    main()
