"""Force a nearby NPC to walk (/moveto go <path>) and prove the controller moves."""
import json
import math
import re
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
ART = Path(r"C:/Users/stsiakal/AppData/Local/Temp/opencode/force-mcp")
USER = "rmsmooth1"
VT_RVA = 0x9B3308
GAME_RVA = 0x15CB1D8
OFF = (6144.0, 11264.0)

sys.path.insert(0, str(ROOT / "tools/feel-recorder"))
from run import Driver  # noqa: E402
from live_mount import Client  # noqa: E402
from live_npc import Mem, fields, cli  # noqa: E402


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    d = Driver([T, "mcp", "--dll", DLL, "--client", CLIENT, "--launcher", LAUNCHER,
                "--password-file", PW, "--login", "127.0.0.1:28237", "--user", USER,
                "--artifact-dir", str(ART)])
    c = Client(d, USER, "Rmsmootho")
    try:
        c.launch()
        c.chat("/move 10460 15460 214")
        time.sleep(20)
        res = json.loads(d.text("client_entities", pid=c.pid, limit=200))
        rows = []
        for key, ent in res.get("entities", {}).items():
            p = ent.get("position") or {}
            rows.append([str(key), str(ent.get("name", "")).split(" ")[0], ent.get("name", ""),
                         p.get("x", 0.0), p.get("y", 0.0), p.get("z", 0.0), ent.get("class", "")])
        from collections import Counter
        print("entities", len(rows), Counter(r[6] for r in rows).most_common(12), flush=True)
        base = int(re.search(r"base=0x([0-9a-f]+)", cli("mod", "x2game.dll", pid=c.pid)).group(1), 16)
        raw = cli("peek-global", f"0x{base + GAME_RVA:x}", "ptr", pid=c.pid)
        game = int(raw.split("value=")[-1].strip(), 0)
        mem = Mem(c.pid)
        timer = mem.u32(game + 0x14)
        hits = mem.find_u32(base + VT_RVA)
        cs = [fields(mem, h) for h in hits]
        cs = [x for x in cs if x and x["active"]]
        print("active controllers", len(cs), flush=True)
        # nearest entity to each controller's local position
        best = None
        for f in cs:
            if f["prev_pos"] == [0.0, 0.0, 0.0]:
                continue
            p = f["prev_pos"]
            for e in rows:
                if len(e) < 7 or "npc" not in e[6].lower():
                    continue
                d2 = (float(e[3]) - p[0]) ** 2 + (float(e[4]) - p[1]) ** 2
                if best is None or d2 < best[0]:
                    best = (d2, f, e)
        print("best match", best[0] if best else None, best[1]["addr"] if best else None,
              best[2] if best else None, flush=True)
        if not best or best[0] > 25:
            print("no controller/entity match", flush=True)
            return
        f, e = best[1], best[2]
        print("target", c.ui("X2Unit:SetWatchTarget(%s) return X2Unit:UnitName('watchtarget')" % json.dumps(e[0])), flush=True)
        c.chat("/moveto go loka_level_perform_06")
        time.sleep(0.5)
        rows2 = []
        t0 = time.monotonic()
        while time.monotonic() - t0 < 8.0:
            row = {"t": time.monotonic() - t0, "ft": None, "c": fields(mem, f["addr"]),
                   "u": c.lua("local e=_G.X2WATCH return ''")}
            out = cli("call", f"0x{mem.u32(mem.u32(timer) + 0x30):x}", "thiscall",
                      f"this=0x{timer:x}", 0, pid=c.pid)
            if out:
                m = re.search(r"st0_f32=([-0-9.eE]+)", out)
                if m:
                    row["ft"] = float(m.group(1))
            rows2.append(row)
            time.sleep(0.01)
        (OUT / "forced_npc.json").write_text(json.dumps(rows2))
        path = sum(math.dist(rows2[i]["c"]["target"], rows2[i - 1]["c"]["target"]) for i in range(1, len(rows2)) if rows2[i]["c"] and rows2[i - 1]["c"])
        print("samples", len(rows2), "target path", round(path, 2), flush=True)
    finally:
        if c.pid:
            try:
                d.text("client_stop", pid=c.pid)
            except Exception:
                pass
        d.close()


if __name__ == "__main__":
    main()
