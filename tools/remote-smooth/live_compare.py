"""Side-by-side remote-unit smoothing: original 1.2 observer vs Bevy observer.

Three clients on the move-golden lane at the same moment:

* original observer  rmsmooth1/Rmsmootho (hidden, driver; Lua sampling only)
* Bevy mover         rmsmooth2/Rmsmoothm (headless, scripted walk)
* Bevy observer      movegold2/Movegoldb (headless, --ported-remote-smoothing)

Both observers watch the same moving unit with the same server movement packets, so position
error, jitter and turn smoothness need no replay. The original's GetWorldPos is zone-local; its
samples are shifted by the map offset into the server/Bevy world.

Outputs docs/research/remote-smoothing/live/compare.json plus the raw CSVs and a summary table.
"""
import json
import math
import re
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
BEVY = Path(r"D:/aaemu-target/bevy-pr/debug/aa-client.exe")
LOGIN = "127.0.0.1:28237"
ART = Path(r"C:/Users/stsiakal/AppData/Local/Temp/opencode/compare-mcp")
OBS_USER, OBS_CHAR = "rmsmooth1", "Rmsmootho"
MOVE_USER, MOVE_CHAR = "rmsmooth2", "Rmsmoothm"
WATCH_USER, WATCH_CHAR = "movegold2", "Movegoldb"
# the original's GetWorldPos is zone-local; the server/Bevy world adds this constant
OFFSET = (6144.0, 11265.0, 0.0)
OBSERVER_POS = (10512.0, 15585.0, 232.0)
MOVER_POS = (10504.0, 15585.0, 232.0)
BEVY_POS = (10520.0, 15585.0, 232.0)
# the mover's script, driven through the Bevy script runner after a fixed warm-up
MOVER_SCRIPT = """feel SETUP
lua if tutorial and tutorial.window then tutorial.window:Show(false) end
wait 1
feel START
wait 15
keydown w
wait 1.0
keydown a
wait 0.55
keyup a
wait 0.45
keydown d
wait 0.65
keyup d
wait 0.85
keyup w
wait 0.2
keydown w
wait 1.5
keyup w
wait 0.3
keydown a
wait 0.7
keyup a
feel END
wait 2
quit now
"""
OBSERVER_SCRIPT = """feel SETUP
lua if tutorial and tutorial.window then tutorial.window:Show(false) end
wait 1
feel START
wait 120
feel END
quit now
"""
# An in-client timer samples the mover's drawn pose once per original frame (16 ms game time);
# rows are fetched over MCP only when the run is over, so the game thread is never blocked by us.
ORIGINAL_TIMER = r"""
_G.RS_RUN = false
_G.RS_STOP = false
_G.RS_ROWS = {}
_G.RS_ENT = nil
_G.RS_NAME = "rmsmoothm"
_G.RS_X = 0
_G.RS_Y = 0
local function find()
  -- a disconnected copy of the character can linger at its last position; the live one is
  -- the match nearest the expected spawn
  local best, bestd = nil, 1e30
  for _,e in pairs(System.GetEntities() or {}) do
    if string.lower(e:GetName())==_G.RS_NAME then
      local ok,p = pcall(function() return e:GetWorldPos() end)
      if ok and p then
        local d = (p.x - _G.RS_X)^2 + (p.y - _G.RS_Y)^2
        if d < bestd then best, bestd = e, d end
      end
    end
  end
  return best
end
local function sample()
  local e = _G.RS_ENT
  if e == nil then e = find() _G.RS_ENT = e end
  if e == nil then return nil end
  local ok,p = pcall(function() return e:GetWorldPos() end)
  if not ok or p == nil then return nil end
  local ok2,a = pcall(function() return e:GetAngles() end)
  local ok3,v = pcall(function() return e:GetVelocity() end)
  if not ok2 or a == nil then a = {x=0,y=0,z=0} end
  if not ok3 or v == nil then v = {x=0,y=0,z=0} end
  return string.format('%d|%.6f|%.5f,%.5f,%.5f|%.5f,%.5f,%.5f|%.5f,%.5f,%.5f',
    System.GetFrameID(), System.GetCurrTime(),
    p.x, p.y, p.z, a.x, a.y, a.z, v.x, v.y, v.z)
end
local function tick()
  if _G.RS_STOP then return end
  if _G.RS_RUN then
    local ok,row = pcall(sample)
    if ok and type(row) == 'string' then _G.RS_ROWS[#_G.RS_ROWS+1] = row end
  end
  Script.SetTimer(16, tick)
end
Script.SetTimer(16, tick)
return 'installed'
"""

sys.path.insert(0, str(ROOT / "tools/feel-recorder"))
from run import Driver, stack_run, stack_up  # noqa: E402


def log(*a):
    print("compare:", *a, flush=True)


class Client:
    def __init__(self, d, user, char):
        self.d, self.user, self.char = d, user, char
        self.pid = None

    def lua(self, code, target="system"):
        return self.d.text("client_lua", pid=self.pid, target=target, code=code)

    def ui(self, code):
        return self.lua(code, "ui")

    def launch(self):
        out = self.d.text("client_launch", fast_start=True, fast_pak=True, hidden=True,
                          user=self.user, password_file=PW)
        self.pid = int(re.search(r"pid (\d+)", out).group(1))
        log(self.user, "pid", self.pid)
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
        want = (pos[0] - OFFSET[0], pos[1] - OFFSET[1], pos[2] - OFFSET[2])
        last = ""
        for _ in range(6):
            try:
                self.d.text("client_chat", pid=self.pid, message="/move %.1f %.1f %.1f" % pos)
            except Exception as e:
                last = str(e)[:80]
                time.sleep(1.5)
                continue
            time.sleep(1.5)
            try:
                p = self.entity_position(self.char)
            except Exception:
                p = None
            if p:
                last = "%.1f %.1f %.1f" % (p[0], p[1], p[2])
                if math.dist(p[:3], want) < 25.0:
                    return
        raise RuntimeError(f"{self.user} did not move to {pos} (at {last})")

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


def launch_bevy(script, remote_csv, feel_csv, account, character, pos, legacy):
    cmd = [
        str(BEVY), "--headless", "--mute", "--account", account, "--login", LOGIN,
        "--password-file", PW, "--character", character,
        "--pos", "%.1f,%.1f,%.1f" % pos, "--yaw", "0",
        "--script", str(script),
        "--feel-csv", str(feel_csv), "--feel-fps", "0",
        "--headless-fps", "60", "--size", "640x360",
        "--offscreen-shot", str(OUT / f"bevy-{account}.png"), "--shot-after", "100000",
    ]
    if remote_csv is not None:
        cmd += ["--remote-csv", str(remote_csv)]
    if legacy:
        cmd += ["--ported-remote-smoothing"]
    log("bevy:", " ".join(cmd))
    logf = (OUT / f"bevy-{account}.log").open("w", encoding="utf-8")
    proc = subprocess.Popen(cmd, cwd=str(ROOT), stdout=logf, stderr=subprocess.STDOUT)
    return proc, logf


def read_remote_csv(path):
    import csv
    rows = []
    if not path.exists():
        return rows
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            try:
                rows.append({
                    "frame": int(row["frame"]), "ms": float(row["ms"]),
                    "wall": float(row["wall_ms"]), "obj": int(row["obj_id"]),
                    "character": row["character"] == "1", "name": row["name"],
                    "x": float(row["x"]), "y": float(row["y"]), "z": float(row["z"]),
                    "yaw": float(row["yaw"]),
                    "target_x": float(row.get("target_x") or 0.0),
                    "target_y": float(row.get("target_y") or 0.0),
                    "target_z": float(row.get("target_z") or 0.0),
                    "vel_x": float(row.get("vel_x") or 0.0),
                    "vel_y": float(row.get("vel_y") or 0.0),
                    "vel_z": float(row.get("vel_z") or 0.0),
                    "anim": float(row.get("anim_speed") or 1.0),
                })
            except (KeyError, ValueError):
                continue
    return rows


def read_feel_csv(path):
    import csv
    rows = []
    if not path.exists():
        return rows
    with path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            try:
                rows.append({
                    "frame": int(row["frame"]), "wall": float(row["wall_ms"]),
                    "ms": float(row["ms"]),
                    "x": float(row["player_x"]), "y": float(row["player_y"]),
                    "z": float(row["player_z"]), "yaw": float(row["player_yaw"]),
                })
            except (KeyError, ValueError):
                continue
    return rows


def resample(rows, t0, t1, hz, get):
    """Linear interpolation of a per-row tuple at a uniform wall-time grid."""
    xs = []
    t = t0
    i = 0
    rows = sorted(rows, key=lambda r: r[0])
    while t <= t1 and rows:
        while i + 1 < len(rows) and rows[i + 1][0] < t:
            i += 1
        if t <= rows[0][0]:
            xs.append((t, *get(rows[0])))
        elif i + 1 >= len(rows):
            xs.append((t, *get(rows[-1])))
        else:
            (ta, *a), (tb, *b) = rows[i], rows[i + 1]
            u = 0.0 if tb <= ta else (t - ta) / (tb - ta)
            xs.append((t, *(x + (y - x) * u for x, y in zip(a, b))))
        t += 1000.0 / hz
    return xs


def unwrap(seq):
    out = []
    last = None
    for y in seq:
        if last is None:
            out.append(y)
        else:
            d = (y - last + math.pi) % (2.0 * math.pi) - math.pi
            out.append(out[-1] + d)
        last = y
    return out


def movement_metrics(samples, t0, t1, hz=60.0):
    """samples: [(wall_ms, x, y, z, yaw, game_ms)]; window by wall, grid by game time."""
    def stat(v):
        if not v:
            return {"mean": 0.0, "sd": 0.0, "p95": 0.0, "max": 0.0}
        m = sum(v) / len(v)
        sd = math.sqrt(sum((x - m) ** 2 for x in v) / len(v))
        s = sorted(v)
        return {"mean": m, "sd": sd, "p95": s[int(0.95 * (len(s) - 1))], "max": max(v)}

    sel = sorted([r for r in samples if t0 <= r[0] <= t1], key=lambda r: r[5])
    if len(sel) < 2:
        return {"samples": 0, "speed": stat([]), "accel": stat([]),
                "yaw_rate_abs": stat([]), "turning_rate": stat([]), "yaw_rate_sd_ratio": 0.0}
    grid = []
    j = 0
    t = sel[0][5]
    while t <= sel[-1][5]:
        while j + 1 < len(sel) and sel[j + 1][5] < t:
            j += 1
        if j + 1 >= len(sel):
            grid.append((t, sel[-1][1], sel[-1][2], sel[-1][4]))
        else:
            (ta, xa, ya, _, wa, gma), (tb, xb, yb, _, wb, gmb) = sel[j], sel[j + 1]
            u = 0.0 if gmb <= gma else (t - gma) / (gmb - gma)
            grid.append((t, xa + (xb - xa) * u, ya + (yb - ya) * u,
                         wa + ((wb - wa + math.pi) % (2.0 * math.pi) - math.pi) * u))
        t += 1000.0 / hz
    yaws = unwrap([g[3] for g in grid])
    speed, accel, yaw_rate = [], [], []
    for i in range(1, len(grid)):
        dt = (grid[i][0] - grid[i - 1][0]) / 1000.0
        if dt <= 0:
            continue
        dx = grid[i][1] - grid[i - 1][1]
        dy = grid[i][2] - grid[i - 1][2]
        speed.append(math.hypot(dx, dy) / dt)
        yaw_rate.append((yaws[i] - yaws[i - 1]) / dt)
    for i in range(1, len(speed)):
        dt = (grid[i][0] - grid[i - 1][0]) / 1000.0
        if dt > 0:
            accel.append((speed[i] - speed[i - 1]) / dt)

    abs_rate = [abs(r) for r in yaw_rate]
    turning = [abs(r) for r in yaw_rate if abs(r) > 0.15]
    return {
        "samples": len(grid),
        "speed": stat(speed),
        "accel": stat([abs(a) for a in accel]),
        "yaw_rate_abs": stat(abs_rate),
        "turning_rate": stat(turning),
        "yaw_rate_sd_ratio": (stat(yaw_rate)["sd"] / stat(abs_rate)["mean"]) if abs_rate and stat(abs_rate)["mean"] > 1e-6 else 0.0,
    }


def pair_error(a, b, t0, t1, hz=60.0):
    """Horizontal distance between two sampled trajectories at a uniform grid."""
    ga = resample(a, t0, t1, hz, lambda r: (r[1], r[2], r[3], r[4]))
    gb = resample(b, t0, t1, hz, lambda r: (r[1], r[2], r[3], r[4]))
    n = min(len(ga), len(gb))
    dist = [math.hypot(ga[i][1] - gb[i][1], ga[i][2] - gb[i][2]) for i in range(n)]
    if not dist:
        return {"mean": 0.0, "p95": 0.0, "max": 0.0}
    s = sorted(dist)
    return {"mean": sum(dist) / len(dist), "p95": s[int(0.95 * (len(s) - 1))], "max": max(dist)}


def yaw_error(a, b, t0, t1, hz=60.0):
    ga = resample(a, t0, t1, hz, lambda r: (r[1], r[2], r[3], r[4]))
    gb = resample(b, t0, t1, hz, lambda r: (r[1], r[2], r[3], r[4]))
    n = min(len(ga), len(gb))
    d = [abs((ga[i][4] - gb[i][4] + math.pi) % (2.0 * math.pi) - math.pi) for i in range(n)]
    if not d:
        return {"mean_deg": 0.0, "p95_deg": 0.0, "max_deg": 0.0}
    s = sorted(d)
    return {"mean_deg": math.degrees(sum(d) / len(d)),
            "p95_deg": math.degrees(s[int(0.95 * (len(s) - 1))]),
            "max_deg": math.degrees(max(d))}


def moving_window(rows, threshold=0.8):
    """The longest contiguous run where a trajectory moves (`rows` are wall, x, y, ..., game_ms)."""
    runs = []
    run_start = None
    for i in range(1, len(rows)):
        dt = (rows[i][5] - rows[i - 1][5]) / 1000.0
        fast = dt > 0 and math.hypot(rows[i][1] - rows[i - 1][1], rows[i][2] - rows[i - 1][2]) / dt > threshold
        if fast and run_start is None:
            run_start = i
        elif not fast and run_start is not None:
            runs.append((run_start, i))
            run_start = None
    if run_start is not None:
        runs.append((run_start, len(rows) - 1))
    if not runs:
        return rows[0][0], rows[-1][0]
    a, b = max(runs, key=lambda r: rows[r[1]][5] - rows[r[0]][5])
    return rows[a][0], rows[b][0]


def fetch_rows(client, chunk=1000):
    total = int(client.lua("return #_G.RS_ROWS").strip() or 0)
    rows = []
    for start in range(1, total + 1, chunk):
        end = min(total, start + chunk - 1)
        text = client.lua("return table.concat(_G.RS_ROWS, '\\n', %d, %d)" % (start, end))
        if text:
            rows.extend(text.splitlines())
    return rows


def parse_original_rows(rows, wall_run):
    out = []
    t0 = None
    for line in rows:
        parts = line.split("|")
        if len(parts) != 5:
            continue
        try:
            t = float(parts[1])
            p = [float(x) for x in parts[2].split(",")]
            a = [float(x) for x in parts[3].split(",")]
        except ValueError:
            continue
        if t0 is None:
            t0 = t
        out.append({"wall": wall_run + (t - t0) * 1000.0, "p": [p[0], p[1], p[2], a[2]], "t": t * 1000.0})
    return out


def wait_for(what, proc, timeout, ready):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc is not None and proc.poll() is not None:
            raise RuntimeError(f"{what}: client exited early")
        if ready():
            return
        time.sleep(1.0)
    raise RuntimeError(f"{what}: timed out")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    ART.mkdir(parents=True, exist_ok=True)
    if not stack_up():
        log("starting the lane stack")
        stack_run("start", timeout=300)
    else:
        log("lane stack already up")
    cmd = [T, "mcp", "--dll", DLL, "--client", CLIENT, "--launcher", LAUNCHER,
           "--password-file", PW, "--login", LOGIN, "--user", OBS_USER,
           "--artifact-dir", str(ART)]
    d = Driver(cmd)
    procs = []
    logs = []
    wall_run = 0.0
    try:
        obs = Client(d, OBS_USER, OBS_CHAR)
        log("launching original observer")
        obs.launch()
        obs.place(OBSERVER_POS)
        try:
            obs.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass
        log("original timer:", obs.lua(ORIGINAL_TIMER).strip())

        script = OUT / "bevy-observer-script.txt"
        script.write_text(OBSERVER_SCRIPT, encoding="utf-8")
        remote_csv, feel_csv = OUT / "ours_remote.csv", OUT / "ours_feel.csv"
        for p in (remote_csv, feel_csv, Path(str(feel_csv) + ".marks.csv")):
            p.unlink(missing_ok=True)
        log("launching Bevy observer")
        proc_obs, log_obs = launch_bevy(script, remote_csv, feel_csv,
                                        WATCH_USER, WATCH_CHAR, BEVY_POS, legacy=True)
        procs.append(proc_obs)
        logs.append(log_obs)

        def bevy_in_world():
            return len(read_remote_csv(remote_csv)) >= 60
        wait_for("Bevy observer in world", proc_obs, 300, bevy_in_world)
        log("Bevy observer is in world;", len(read_remote_csv(remote_csv)), "rows")

        mover_script = OUT / "bevy-mover-script.txt"
        mover_script.write_text(MOVER_SCRIPT, encoding="utf-8")
        mover_csv = OUT / "mover_feel.csv"
        for p in (mover_csv, Path(str(mover_csv) + ".marks.csv")):
            p.unlink(missing_ok=True)
        log("launching Bevy mover")
        proc_mov, log_mov = launch_bevy(mover_script, None, mover_csv,
                                        MOVE_USER, MOVE_CHAR, MOVER_POS, legacy=False)
        procs.append(proc_mov)
        logs.append(log_mov)

        def observer_sees_mover():
            mine = [r for r in read_remote_csv(remote_csv) if r["name"].lower() == MOVE_CHAR.lower()]
            if not mine:
                return False
            last = mine[-1]
            return math.hypot(last["x"] - MOVER_POS[0], last["y"] - MOVER_POS[1]) < 3.0
        wait_for("Bevy observer sees the mover at its start", proc_mov, 240, observer_sees_mover)
        wall_run = time.time() * 1000.0
        obs.lua("_G.RS_ENT = nil _G.RS_X = %.3f _G.RS_Y = %.3f _G.RS_RUN = true return 1"
                % (MOVER_POS[0] - OFFSET[0], MOVER_POS[1] - OFFSET[1]))
        log("Bevy observer sees the mover; original timer running")
        try:
            proc_mov.wait(timeout=180)
        except subprocess.TimeoutExpired:
            log("mover did not quit; terminating")
        time.sleep(1.5)
        obs.lua("_G.RS_STOP = true _G.RS_RUN = false return #_G.RS_ROWS")
        samples = parse_original_rows(fetch_rows(obs), wall_run)
        rows = read_remote_csv(remote_csv)
        (OUT / "original_observer.json").write_text(json.dumps(samples), encoding="utf-8")
        log("original samples", len(samples), "bevy rows", len(rows))
        return analyze(samples, rows, mover_csv)
    finally:
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


def analyze(samples, rows, mover_csv):
    ours = [r for r in rows if r["name"].lower() == MOVE_CHAR.lower()]
    if len(ours) < 120:
        raise RuntimeError(f"too few Bevy rows for {MOVE_CHAR}: {len(ours)}")
    original = [(r["wall"], r["p"][0] + OFFSET[0], r["p"][1] + OFFSET[1], r["p"][2] + OFFSET[2], r["p"][3], r["t"])
                for r in samples]
    if len(original) < 120:
        raise RuntimeError(f"too few original samples: {len(original)}")
    bevy = [(r["wall"], r["x"], r["y"], r["z"], r["yaw"], r["ms"]) for r in ours]
    mover = [(r["wall"], r["x"], r["y"], r["z"], r["yaw"], r["ms"]) for r in read_feel_csv(mover_csv)]
    if len(mover) < 60:
        raise RuntimeError(f"no mover path in {mover_csv}")
    # the moving window is the longest run where the mover itself walks; a spawn/teleport
    # produces one or two fast frames and is skipped
    t_start = mover[0][0]
    t_end = mover[-1][0]
    runs = []
    run_start = None
    for i in range(1, len(mover)):
        dt = (mover[i][5] - mover[i - 1][5]) / 1000.0
        fast = dt > 0 and math.hypot(mover[i][1] - mover[i - 1][1], mover[i][2] - mover[i - 1][2]) / dt > 0.8
        if fast and run_start is None:
            run_start = i
        elif not fast and run_start is not None:
            runs.append((run_start, i))
            run_start = None
    if run_start is not None:
        runs.append((run_start, len(mover) - 1))
    if runs:
        a, b = max(runs, key=lambda r: mover[r[1]][5] - mover[r[0]][5])
        t_start, t_end = mover[a][0], mover[b][0]
    m_orig = movement_metrics(original, t_start, t_end)
    m_bevy = movement_metrics(bevy, t_start, t_end)
    m_path = movement_metrics(mover, t_start, t_end)
    # The two clients sample at different rates (e.g. 45.8 vs 53.7 Hz). The
    # 60 Hz grid above keeps the sources' own event times; this second view
    # resamples both to the same low rate so a run is comparable across loads.
    common_hz = 20.0
    m_orig_common = movement_metrics(original, t_start, t_end, hz=common_hz)
    m_bevy_common = movement_metrics(bevy, t_start, t_end, hz=common_hz)
    m_path_common = movement_metrics(mover, t_start, t_end, hz=common_hz)
    result = {
        "window": {"start_wall_ms": t_start, "end_wall_ms": t_end, "seconds": (t_end - t_start) / 1000.0},
        "clock": {
            "original_hz": len([s for s in samples if t_start <= s["wall"] <= t_end]) / ((t_end - t_start) / 1000.0),
            "bevy_hz": len([r for r in ours if t_start <= r["wall"] <= t_end]) / ((t_end - t_start) / 1000.0),
        },
        "position_error": {
            "ours_vs_original": pair_error(bevy, original, t_start, t_end),
            "original_vs_path": pair_error(original, mover, t_start, t_end),
            "ours_vs_path": pair_error(bevy, mover, t_start, t_end),
        },
        "turn_error": {
            "ours_vs_original": yaw_error(bevy, original, t_start, t_end),
            "original_vs_path": yaw_error(original, mover, t_start, t_end),
            "ours_vs_path": yaw_error(bevy, mover, t_start, t_end),
        },
        "movement": {"original": m_orig, "bevy": m_bevy, "mover_path": m_path},
        "movement_common": {
            "hz": common_hz,
            "original": m_orig_common,
            "bevy": m_bevy_common,
            "mover_path": m_path_common,
        },
    }
    (OUT / "compare.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))
    log("wrote", OUT / "compare.json")
    return result


if __name__ == "__main__":
    main()
