"""Side-by-side remote NPC smoothing: original 1.2 observer vs Bevy observer.

The original 1.2 client drives a village NPC along a recorded move path (`/moveto go`); the
Bevy observer draws the same NPC through the ported `NetActorMovementController`
(`--ported-remote-smoothing`). Both observers are on the move-golden lane at the same time, so
they receive the same server movement packets for the same unit.

The original side is read two ways without blocking its game thread:

* an in-client Lua timer (60 Hz) samples the NPC's drawn `GetWorldPos`/`GetAngles`;
* a Python thread reads the NPC's controller fields (`+0x430` target, `+0x4b0` applied, `+0x400`
  filter velocity, `+0x410` animation speed) at ~100 Hz through ReadProcessMemory.

Outputs docs/research/remote-smoothing/live/npc_compare.json plus the raw CSVs.
"""
import json
import math
import re
import subprocess
import sys
import threading
import time

sys.path.insert(0, r"D:/aaemu-client/wt/pr/tools/remote-smooth")
sys.path.insert(0, r"D:/aaemu-client/wt/pr/tools/feel-recorder")
import live_compare as lc
import live_capture as cap
from run import Driver

NPC_NAME = "Burin"
NPC_CLASS = "npc_daru_tamer"
MOVE_PATH = "loka_level_perform_06"
SAMPLE_SECONDS = 16.0
OUT = lc.OUT

TIMER = lc.ORIGINAL_TIMER


def cli(pid, *args):
    p = subprocess.run([lc.T, *map(str, args), "--pid", str(pid)], capture_output=True, text=True, timeout=120)
    if p.returncode != 0:
        raise RuntimeError(p.stderr.strip() or p.stdout.strip())
    return p.stdout.strip()


def peek(pid, addr):
    return int(cli(pid, "peek-global", "0x%x" % addr, "ptr").strip(), 0)


def unit_origin(pid):
    """The native movement mode getter (documented in the report; 3 = extrapolate branch)."""
    base = int(re.search(r"base=0x([0-9a-f]+)", cli(pid, "mod", "x2game.dll")).group(1), 16)
    obj = peek(pid, base + 0xAF4E80)
    vt = peek(pid, obj)
    fn = peek(pid, vt + 0x8)
    out = cli(pid, "call", "0x%x" % fn, "thiscall", "this=0x%x" % obj)
    m = re.search(r"eax=(0x[0-9a-fA-F]+)", out)
    return int(m.group(1), 16) if m else None


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    lc.ART.mkdir(parents=True, exist_ok=True)
    if not lc.stack_up():
        print("compare: starting the lane stack", flush=True)
        lc.stack_run("start", timeout=300)
    d = Driver([lc.T, "mcp", "--dll", lc.DLL, "--client", lc.CLIENT, "--launcher", lc.LAUNCHER,
                "--password-file", lc.PW, "--login", lc.LOGIN, "--user", lc.OBS_USER,
                "--artifact-dir", str(lc.ART)])
    procs, logs = [], []
    stop = threading.Event()
    samples = []
    thread = None
    try:
        obs = lc.Client(d, lc.OBS_USER, lc.OBS_CHAR)
        print("compare: launching original observer", flush=True)
        obs.launch()
        obs.place(lc.OBSERVER_POS)
        try:
            obs.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass
        print("compare: original movement mode", unit_origin(obs.pid), flush=True)
        print("compare: original timer", obs.lua(TIMER).strip(), flush=True)

        script = OUT / "npc-observer-script.txt"
        script.write_text(lc.OBSERVER_SCRIPT, encoding="utf-8")
        remote_csv = OUT / "npc_ours_remote.csv"
        feel_csv = OUT / "npc_ours_feel.csv"
        for p in (remote_csv, feel_csv, remote_csv.with_suffix(".json")):
            p.unlink(missing_ok=True)
        print("compare: launching Bevy observer", flush=True)
        proc, logf = lc.launch_bevy(script, remote_csv, feel_csv, lc.WATCH_USER, lc.WATCH_CHAR,
                                    lc.BEVY_POS, legacy=True)
        procs.append(proc)
        logs.append(logf)

        def in_world():
            return len(lc.read_remote_csv(remote_csv)) >= 60
        lc.wait_for("Bevy observer in world", proc, 300, in_world)

        # spawn a fresh daru tamer next to the observers: an NPC with the AI that /moveto drives
        px, py = lc.OBSERVER_POS[0] - lc.OFFSET[0], lc.OBSERVER_POS[1] - lc.OFFSET[1]
        try:
            d.text("client_chat", pid=obs.pid, message="/spawn npc 6396")
            print("compare: spawned daru tamer", flush=True)
        except Exception as e:
            print("compare: spawn failed:", str(e)[:120], flush=True)
        time.sleep(2.0)
        code = ("local out={} local px,py=%.1f,%.1f "
                "for _,e in pairs(System.GetEntities() or {}) do "
                "if string.sub(tostring(e.class),1,4)=='npc_' then "
                "local ok,p=pcall(function() return e:GetWorldPos() end) "
                "if ok and p then local d=math.sqrt((p.x-px)^2+(p.y-py)^2) "
                "if d<80 then out[#out+1]=string.format('%%s|%%s|%%.1f|%%.1f|%%.1f|%%.1f', "
                "tostring(e:GetName()), tostring(e.class), d, p.x, p.y, p.z) end end end end "
                "return table.concat(out, ';')") % (px, py)
        rows = []
        for _ in range(15):
            rows = [r.split("|") for r in obs.lua(code).split(";") if r]
            rows.sort(key=lambda r: float(r[2]))
            if rows and float(rows[0][2]) < 25.0:
                break
            time.sleep(1.0)
        if not rows:
            raise RuntimeError("no NPC within 80 m of the original observer")
        daru = [r for r in rows if r[1] == "npc_daru_tamer" and float(r[2]) < 30.0]
        pick = daru[0] if daru else rows[0]
        npc_name = pick[0]
        npc_local = tuple(float(v) for v in pick[3:6])
        npc_world = (npc_local[0] + lc.OFFSET[0], npc_local[1] + lc.OFFSET[1], npc_local[2])
        print("compare: npc", npc_name, pick[1], "at", [round(v, 1) for v in npc_world], flush=True)

        # its server object id: the Bevy observer's row at the NPC's position
        for _ in range(20):
            rows = lc.read_remote_csv(remote_csv)
            near = [r for r in rows
                    if math.hypot(r["x"] - npc_world[0], r["y"] - npc_world[1]) < 3.0]
            if near:
                break
            time.sleep(1.0)
        else:
            raise RuntimeError("Bevy observer has no row at the NPC position")
        obj = max(set(r["obj"] for r in near), key=lambda o: sum(1 for r in near if r["obj"] == o))
        print("compare: npc object id", obj, hex(obj), flush=True)
        out = d.text("client_target", pid=obs.pid, unit_id="%x" % obj, expected_name=npc_name)
        print("compare: target", out[:120].replace("\n", " "), flush=True)

        # the NPC's controller: nearest to its zone-local position
        base = int(re.search(r"base=0x([0-9a-f]+)", cli(obs.pid, "mod", "x2game.dll")).group(1), 16)
        mem = cap.Mem(obs.pid)
        hits = mem.find_u32(base + cap.CONTROLLER_VTABLE_RVA)
        best = None
        for h in hits:
            f = cap.controller_fields(mem, h)
            if f:
                dist = math.dist(f["prev_pos"], npc_local)
                if best is None or dist < best[0]:
                    best = (dist, f["addr"], f)
        if best is None or best[0] > 8.0:
            raise RuntimeError(f"no controller near {npc_name} (best {best[0] if best else None})")
        print("compare: controller", best[1], "dist", round(best[0], 2), flush=True)
        ctl_addr = best[1]
        start_wall = time.time() * 1000.0

        def sample_controller():
            t0 = time.monotonic()
            while not stop.is_set():
                t = time.monotonic() - t0
                f = cap.controller_fields(mem, ctl_addr)
                if f:
                    samples.append({"wall": start_wall + t * 1000.0,
                                    "target": f["target"], "prev": f["prev_pos"],
                                    "vel": f["vel"], "step": f["step_vel"],
                                    "anim": f["anim_speed"], "turn": f["turn_vel"]})
                time.sleep(0.008)
        thread = threading.Thread(target=sample_controller, daemon=True)
        thread.start()

        obs.lua("_G.RS_NAME = %s _G.RS_ENT = nil _G.RS_X = %.3f _G.RS_Y = %.3f _G.RS_RUN = true return 1"
                % (json.dumps(npc_name.lower()), npc_local[0], npc_local[1]))
        wall_run = time.time() * 1000.0
        for cmd in ("/ai load_path %s" % MOVE_PATH, "/ai set_behavior FollowPath",
                    "/moveto go %s" % MOVE_PATH):
            d.text("client_chat", pid=obs.pid, message=cmd)
            time.sleep(0.5)
        print("compare: NPC walking; sampling", flush=True)
        time.sleep(SAMPLE_SECONDS)
        stop.set()
        thread.join(timeout=10)
        obs.lua("_G.RS_STOP = true _G.RS_RUN = false return #_G.RS_ROWS")
        lua_rows = lc.parse_original_rows(lc.fetch_rows(obs), wall_run)
        (OUT / "npc_original.json").write_text(json.dumps({"controller": samples, "drawn": lua_rows}), encoding="utf-8")
        return analyze(npc_name, obj, samples, lua_rows, lc.read_remote_csv(remote_csv))
    finally:
        stop.set()
        if thread is not None:
            thread.join(timeout=10)
        try:
            if 'obs' in locals() and obs.pid:
                d.text("client_stop", pid=obs.pid)
        except Exception:
            pass
        for proc in procs:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=15)
                except Exception:
                    proc.kill()
        for logf in logs:
            logf.close()
        d.close()


def analyze(npc_name, obj, ctl, drawn, rows):
    ours = [r for r in rows if r["obj"] == obj]
    if len(ours) < 300:
        raise RuntimeError(f"too few Bevy rows for object {obj}: {len(ours)}")
    if len(drawn) < 100 or len(ctl) < 100:
        raise RuntimeError(f"too few original rows: drawn {len(drawn)} controller {len(ctl)}")
    # the window is the longest run of the original controller target moving
    target = [(r["wall"], r["target"][0], r["target"][1], r["target"][2], 0.0, r["wall"]) for r in ctl]
    t_start, t_end = lc.moving_window(target)
    if (t_end - t_start) < 2000.0:
        raise RuntimeError(f"the NPC did not walk (window {(t_end - t_start) / 1000.0:.2f} s)")
    orig = [(r["wall"], r["p"][0] + lc.OFFSET[0], r["p"][1] + lc.OFFSET[1], r["p"][2] + lc.OFFSET[2],
             r["p"][3], r["t"]) for r in drawn]
    bevy = [(r["wall"], r["x"], r["y"], r["z"], r["yaw"], r["ms"]) for r in ours]
    result = {
        "npc": npc_name,
        "object_id": obj,
        "window": {"start_wall_ms": t_start, "end_wall_ms": t_end, "seconds": (t_end - t_start) / 1000.0},
        "clock": {
            "original_hz": len([r for r in drawn if t_start <= r["wall"] <= t_end]) / ((t_end - t_start) / 1000.0),
            "bevy_hz": len([r for r in ours if t_start <= r["wall"] <= t_end]) / ((t_end - t_start) / 1000.0),
        },
        "position_error": {
            "ours_vs_original": lc.pair_error(bevy, orig, t_start, t_end),
            "original_vs_target": lc.pair_error(orig, target, t_start, t_end),
            "ours_vs_target": lc.pair_error(bevy, target, t_start, t_end),
        },
        "turn_error": {
            "ours_vs_original": lc.yaw_error(bevy, orig, t_start, t_end),
        },
        "movement": {
            "original": lc.movement_metrics(orig, t_start, t_end),
            "bevy": lc.movement_metrics(bevy, t_start, t_end),
        },
        "controller": {
            "anim_speed": {"original": ctl[len(ctl) // 2]["anim"], "bevy": ours[len(ours) // 2]["anim"]},
            "target_match": lc.pair_error(
                [(r["wall"], r["target"][0], r["target"][1], r["target"][2], 0.0, r["wall"]) for r in ctl],
                [(r["wall"], r["target_x"], r["target_y"], r["target_z"], 0.0, r["ms"]) for r in ours],
                t_start, t_end),
        },
    }
    (OUT / "npc_compare.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))
    print("compare: wrote", OUT / "npc_compare.json", flush=True)
    return result


if __name__ == "__main__":
    main()
