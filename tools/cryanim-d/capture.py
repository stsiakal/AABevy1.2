#!/usr/bin/env python3
"""Capture the mounted rider's animation selection, LMG descriptor and seat, step D.

Launches the hidden 1.2 client (lane `anim-golden`), mounts the horse, drives idle / gallop /
Q+W / E+W / A turn / stop / jump with the client's own input path and samples, read-only,
through the driver:

* the local player's and the mount's layer-0 FIFO entry and the `SAnimation` descriptor it
  points at (`entry+0`): ids, per-example weights at `+0xa4`, direction `+0x148/+0x14c`,
  tier `+0x144`, `+0x150`, `+0x154`;
* the `CAttachmentManager` objects of both characters, including the rider type-2 object's
  model-space seat `QuatT` at `+0x230` and world `QuatT` at `+0x24c`;
* the animation-set name behind every current clip (`CAnimationSet` entry `+0` string);
* the Lua animation state (`GetCurrentAnimationState`).

After the scenario it calls the pure LMG leaves on scratch descriptors (`FUN_31565d20`,
`FUN_3155c710`) for 20+ synthetic inputs per function. Output JSON feeds the port tests.
"""
import argparse
import json
import re
import struct
import sys
import time
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parents[1]
sys.path.insert(0, str(ROOT / "tools" / "cryanim-b"))
sys.path.insert(0, str(ROOT / "tools" / "feel-recorder"))

import capture as b  # noqa: E402
import run as feel  # noqa: E402

CRYANIMATION_BASE = 0x31500000
X2_BASE = 0x38FF0000
FUN_3160CC50 = 0x3160CC50
FUN_31565D20 = 0x31565D20
FUN_3155C710 = 0x3155C710
CAF_STRIDE = 0xDC
LMG_STRIDE = 0x48
LAYERS = 16
LAYER_STRIDE = 0x30
ENTRY_SIZE = 0x88
MAX_ENTRIES = 8
DESC_LEN = 0x260


class Mem:
    def __init__(self, pid):
        self.m = feel.ProcessMemory(pid)
        self.pid = pid

    def read(self, addr, n):
        if not addr:
            return None
        return self.m.read(addr, n)

    def u32(self, addr):
        return self.m.u32(addr)

    def u16(self, addr):
        raw = self.read(addr, 2)
        return struct.unpack("<H", raw)[0] if raw and len(raw) == 2 else 0

    def f32(self, addr):
        raw = self.read(addr, 4)
        return struct.unpack("<f", raw)[0] if raw and len(raw) == 4 else 0.0

    def cstr(self, addr, limit=96):
        raw = self.read(addr, limit) if addr else None
        if not raw:
            return ""
        return raw.split(b"\0", 1)[0].decode("latin1", "replace")


class Calls:
    def __init__(self, exe, pid):
        self.exe = str(exe)
        self.pid = str(pid)

    def run(self, *args, timeout=60):
        cmd = [self.exe, *[str(a) for a in args], "--pid", self.pid]
        out = feel.subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        if out.returncode != 0:
            raise RuntimeError("driver %s: %s %s" % (" ".join(map(str, args)), out.stdout, out.stderr))
        return out.stdout.strip()

    def call(self, module, va, conv="thiscall", this=None, args=()):
        mod = "x2game.dll" if module == "x2game" else "cryanimation.dll"
        base = X2_BASE if module == "x2game" else CRYANIMATION_BASE
        spec = ["call", "%s+%x" % (mod, va - base), conv]
        if this is not None:
            spec.append("this=0x%08x" % this)
        spec.append("void")
        spec.extend(str(a) for a in args)
        out = self.run(*spec)
        m = re.search(r"eax=0x([0-9a-fA-F]+)", out)
        return int(m.group(1), 16) if m else 0

    def alloc(self, size):
        out = self.run("alloc", size)
        m = re.search(r"0x([0-9a-fA-F]+)", out)
        return int(m.group(1), 16) if m else 0

    def free(self, addr):
        self.run("free", hex(addr))

    def write(self, addr, data):
        self.run("write", hex(addr), data.hex())

    def read(self, addr, size):
        out = self.run("read", hex(addr), size)
        m = re.search(r"(?:^|\s)((?:[0-9a-fA-F]{2})+)", out)
        return bytes.fromhex(m.group(1)) if m else b""


def anim_set(mem, anim):
    char = mem.u32(anim + 0x498) if anim else 0
    holder = mem.u32(char + 0x180) if char else 0
    return holder + 0x5D4 if holder else 0


def entry_names(calls, mem, set_ptr, clip_id):
    if not set_ptr or clip_id < 0:
        return None
    rec = calls.call("cryanimation", FUN_3160CC50, this=set_ptr, args=(clip_id,))
    if not rec:
        return None
    name = mem.cstr(mem.u32(rec))
    raw = mem.read(rec, 0x10)
    index, kind = struct.unpack_from("<IH", raw, 8) if raw else (0, 0)
    return {"record": rec, "name": name, "index": index, "kind": kind, "raw": raw.hex() if raw else ""}


def desc_info(mem, desc):
    raw = mem.read(desc, DESC_LEN)
    if not raw:
        return None
    first_id, count, limit = struct.unpack_from("<HHH", raw, 0)
    count = min(count, 40)
    ids = list(struct.unpack_from("<%dH" % count, raw, 4))
    weights = list(struct.unpack_from("<%df" % count, raw, 0xA4))
    return {
        "address": hex(desc),
        "raw": raw.hex(),
        "first_id": first_id,
        "count": count,
        "limit": limit,
        "ids": ids,
        "weights": weights,
        "tier": struct.unpack_from("<f", raw, 0x144)[0],
        "dir": list(struct.unpack_from("<2f", raw, 0x148)),
        "v150": struct.unpack_from("<f", raw, 0x150)[0],
        "v154": struct.unpack_from("<f", raw, 0x154)[0],
        "v158": struct.unpack_from("<f", raw, 0x158)[0],
    }


def read_anim(mem, anim, calls, names=None):
    if not anim:
        return None
    out = {"object": anim, "layers": [], "names": {}}
    names = names if names is not None else {}
    set_ptr = anim_set(mem, anim)
    out["set"] = set_ptr
    for i in range(LAYERS):
        raw = mem.read(anim + 0xCC + i * LAYER_STRIDE, LAYER_STRIDE)
        if not raw:
            out["layers"].append(None)
            continue
        start, end, _cap = struct.unpack_from("<III", raw, 0)
        layer = {"header": raw.hex(), "entries": [], "descs": []}
        if 0x3B0000 < start < end < 0x80000000:
            n = min((end - start) // ENTRY_SIZE, MAX_ENTRIES)
            for j in range(n):
                raw_entry = mem.read(start + j * ENTRY_SIZE, ENTRY_SIZE)
                if not raw_entry:
                    continue
                layer["entries"].append(raw_entry.hex())
                asset = struct.unpack_from("<I", raw_entry, 0)[0]
                layer["descs"].append(desc_info(mem, asset) if asset else None)
                clip = struct.unpack_from("<h", raw_entry, 8)[0]
                if i == 0 and clip >= 0:
                    if clip not in names:
                        names[clip] = entry_names(calls, mem, set_ptr, clip)
                    out["names"][clip] = names[clip]
        out["layers"].append(layer)
    return out


def read_manager(mem, char):
    if not char:
        return None
    mgr = char + 0x30
    start = mem.u32(mgr + 0x08)
    end = mem.u32(mgr + 0x0C)
    objects = []
    if 0x3B0000 < start <= end < 0x80000000 and 0 < end - start <= 0x100 * 4:
        for j in range((end - start) // 4):
            p = mem.u32(start + j * 4)
            if not (0x3B0000 < p < 0x80000000):
                continue
            raw = mem.read(p, 0x2C0)
            if not raw:
                continue
            q = lambda off: list(struct.unpack_from("<4f", raw, off))
            v = lambda off: list(struct.unpack_from("<3f", raw, off))
            objects.append({
                "address": hex(p),
                "type": struct.unpack_from("<i", raw, 0xB8)[0],
                "bone": struct.unpack_from("<i", raw, 0xC4)[0],
                "align": struct.unpack_from("<i", raw, 0xD4)[0],
                "local_q": q(0x230), "local_p": v(0x240),
                "world_q": q(0x24C), "world_p": v(0x25C),
                "raw": raw.hex(),
            })
    return {"manager": hex(mgr), "objects": objects}


def model_transform(calls, mem, model, models, name):
    if not model:
        return None
    key = name + "_tf"
    p = models.get(key, 0)
    if not p:
        fn = mem.u32(mem.u32(model) + 0x228)
        if not fn:
            return None
        p = calls.call("x2game", fn, this=model)
        if not p:
            return None
        models[key] = p
    raw = mem.read(p, 0x40)
    if not raw:
        return None
    return {"address": hex(p), "raw": raw.hex(),
            "q": list(struct.unpack_from("<4f", raw, 0)),
            "p": list(struct.unpack_from("<3f", raw, 0x10))}


def sample(mem, calls, models, names):
    row = {"t": time.time()}
    for name in ("player", "mount"):
        model = models.get(name)
        if not model:
            continue
        char, anim = models.get(name + "_char", 0), models.get(name + "_anim", 0)
        if char and not mem.u32(char):
            models.pop(name + "_char", None)
            models.pop(name + "_anim", None)
            continue
        if anim and mem.u32(anim + 0x498) != char:
            char, anim = b.resolve_char(calls, mem, model)
            models[name + "_char"], models[name + "_anim"] = char, anim
        row[name] = {
            "model": model,
            "transform": model_transform(calls, mem, model, models, name),
            "anim": read_anim(mem, anim, calls, names),
            "manager": read_manager(mem, char),
        }
    return row


STATE_LUA = r"""
local function vec(t)
  if type(t) ~= 'table' then return 'nil' end
  return string.format('%.5f,%.5f,%.5f', t.x or 0, t.y or 0, t.z or 0)
end
local function one(k)
  local e = (k == 'player') and _G.FEEL_PE or _G.FEEL_HE
  if e == nil then return 'nil|nil|nil|nil|nil|nil' end
  local st, ang, pos, bone, ms = '', nil, nil, nil, 0
  pcall(function() st = e.actor:GetCurrentAnimationState() or '' end)
  pcall(function() ang = e:GetAngles() end)
  pcall(function() pos = e:GetWorldPos() end)
  pcall(function() bone = e:GetBoneDir(k == 'player' and 'Bip01 Pelvis' or 'Bip01', 0) end)
  pcall(function() ms = e:GetAnimationTime(0, 0) end)
  return table.concat({st, vec(ang), vec(pos), vec(bone), string.format('%.4f', ms or 0)}, '|')
end
return one('player') .. '#' .. one('mount')
"""


def write_scenario(client, phases):
    t0 = time.monotonic()
    for at, until, keys, label in phases:
        delay = at - (time.monotonic() - t0)
        if delay > 0:
            time.sleep(delay)
        if keys:
            client.release()
            duration = max(0.05, until - at) * 1000.0
            for key in keys:
                client.key(key, duration)
    tail = phases[-1][1] + 0.6 - (time.monotonic() - t0)
    if tail > 0:
        time.sleep(tail)


def run_calls(mem, calls, models, result):
    """Pure LMG leaf calls on scratch descriptors (20+ cases each)."""
    set_ptr = anim_set(mem, models.get("player_anim", 0))
    source = None
    for row in reversed(result["samples"]):
        p = row.get("player")
        if not p or not p.get("anim"):
            continue
        for layer in p["anim"]["layers"]:
            if not layer:
                continue
            for d in layer.get("descs") or []:
                if d and d["count"] > 8 and d["first_id"] >= 0:
                    source = d
                    break
            if source:
                break
        if source:
            break
    if not set_ptr or not source:
        result["calls_error"] = "no anim set or no LMG descriptor"
        return
    scratch = calls.alloc(DESC_LEN)
    if not scratch:
        result["calls_error"] = "alloc failed"
        return
    base = bytes.fromhex(source["raw"])
    cases = []
    try:
        directions = [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, -1.0), (0.0, -1.0),
                      (-1.0, -1.0), (-1.0, 0.0), (-1.0, 1.0),
                      (0.3827, 0.9239), (0.7071, 0.7071), (0.9239, 0.3827),
                      (0.0, 0.0), (0.5, 0.5), (-0.5, 0.5), (0.5, -0.5), (-0.5, -0.5)]
        tiers = [-1.0, -0.5, 0.0, 0.5, 1.0]
        turns = [-1.0, -0.5, 0.0, 0.5, 1.0]
        # reproduce the live descriptor's own fields
        for label, fields in (("live", True),):
            calls.write(scratch, base)
            before = calls.read(scratch, DESC_LEN)
            calls.call("cryanimation", FUN_31565D20, "cdecl", args=(hex(set_ptr), hex(scratch)))
            after = calls.read(scratch, DESC_LEN)
            cases.append({"fn": "31565d20", "args": {"live": True}, "before": before.hex(), "after": after.hex()})
        for i in range(24):
            d = directions[i % len(directions)]
            desc = bytearray(base)
            struct.pack_into("<f", desc, 0x144, tiers[i % len(tiers)])
            struct.pack_into("<2f", desc, 0x148, *d)
            struct.pack_into("<f", desc, 0x150, turns[(i // 5) % len(turns)])
            struct.pack_into("<f", desc, 0x154, turns[(i // 3) % len(turns)])
            # clear weights before the call
            desc[0xA4:0x144] = b"\0" * (0x144 - 0xA4)
            calls.write(scratch, bytes(desc))
            calls.call("cryanimation", FUN_31565D20, "cdecl", args=(hex(set_ptr), hex(scratch)))
            after = calls.read(scratch, DESC_LEN)
            cases.append({"fn": "31565d20", "args": {"tier": tiers[i % 5], "dir": list(d),
                                                      "v150": turns[(i // 5) % 5], "v154": turns[(i // 3) % 5]},
                          "before": bytes(desc).hex(), "after": after.hex()})
        calls.write(scratch, base)
        before = calls.read(scratch, DESC_LEN)
        calls.call("cryanimation", FUN_3155C710, "cdecl", args=(hex(scratch),))
        after = calls.read(scratch, DESC_LEN)
        cases.append({"fn": "3155c710", "args": {"live": True}, "before": before.hex(), "after": after.hex()})
        for i in range(20):
            d = directions[i % len(directions)]
            desc = bytearray(base)
            struct.pack_into("<f", desc, 0x144, tiers[i % len(tiers)])
            struct.pack_into("<2f", desc, 0x148, *d)
            struct.pack_into("<f", desc, 0x150, turns[(i // 4) % len(turns)])
            struct.pack_into("<f", desc, 0x154, turns[(i // 2) % len(turns)])
            desc[0xA4:0x144] = b"\0" * (0x144 - 0xA4)
            calls.write(scratch, bytes(desc))
            calls.call("cryanimation", FUN_3155C710, "cdecl", args=(hex(scratch),))
            after = calls.read(scratch, DESC_LEN)
            cases.append({"fn": "3155c710", "args": {"tier": tiers[i % 5], "dir": list(d),
                                                      "v150": turns[(i // 4) % 5], "v154": turns[(i // 2) % 5]},
                          "before": bytes(desc).hex(), "after": after.hex()})
    finally:
        calls.free(scratch)
    result["calls"] = cases
    result["calls_source"] = source


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--driver", default="D:/aaemu-target/anim-golden/live/aaemu-clientdriver.exe")
    ap.add_argument("--dll", default="D:/aaemu-target/anim-golden/aadriver/aadriver.dll")
    ap.add_argument("--client", default="D:/AA1.2/bin32/archeage.exe")
    ap.add_argument("--launcher", default="D:/aaemu-live-anim-golden/bin/aaemu-launcher.exe")
    ap.add_argument("--login", default="127.0.0.1:26237")
    ap.add_argument("--password-file", required=True)
    ap.add_argument("--user", default="animgolden")
    ap.add_argument("--character", default="Animgolda")
    ap.add_argument("--out", required=True)
    ap.add_argument("--period", type=float, default=0.05)
    args = ap.parse_args()

    command = [args.driver, "mcp", "--dll", args.dll, "--client", args.client,
               "--launcher", args.launcher, "--password-file", args.password_file,
               "--login", args.login, "--user", args.user,
               "--artifact-dir", str(Path(args.out) / "mcp")]
    d = feel.Driver(command)
    client = feel.OriginalClient(d, Path(args.password_file), args.user, args.character, Path(args.out))
    result = {"user": args.user, "character": args.character, "samples": [], "states": []}
    try:
        client.launch()
        mem = Mem(client.pid)
        calls = Calls(args.driver, client.pid)
        try:
            client.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass
        client.chat("/move 10512.0 15585.0 232.0")
        time.sleep(2.0)
        base = mem.m.module_base("x2game.dll")
        game = mem.u32(base + (b.GAMECLIENT - X2_BASE))
        unit = mem.u32(game + 0x10C) if game else 0
        player_model = mem.u32(unit + 0x3BE8) if unit else 0
        models = {"player": player_model}
        char, anim = b.resolve_char(calls, mem, player_model)
        if char and not anim:
            anim = b.find_anim_by_vtable(mem.m, char)
        models.update({"player_char": char, "player_anim": anim})
        result["models"] = {k: v for k, v in models.items() if isinstance(v, int)}

        client.ensure_item(8159)
        client.summon(8159)
        mate = calls.call("x2game", b.FUN_3933F5E0, "fastcall", this=base + (b.MATE_MAP - X2_BASE))
        mate_uid = mem.u32(mate + 0x20) if mate else 0
        mate_unit = calls.call("x2game", b.FUN_39360020, "cdecl", args=("0x%08x" % mate_uid,)) if mate_uid else 0
        mount_model = mem.u32(mate_unit + 0x3BE8) if mate_unit else 0
        models["mount"] = mount_model
        mchar, manim = b.resolve_char(calls, mem, mount_model) if mount_model else (0, 0)
        if mchar and not manim:
            manim = b.find_anim_by_vtable(mem.m, mchar)
        models.update({"mount_char": mchar, "mount_anim": manim})
        result["models"] = {k: v for k, v in models.items() if isinstance(v, int)}
        client.mount()

        phases = [
            (0.0, 1.0, [], "mounted_idle"),
            (1.0, 6.0, ["w"], "mounted_gallop"),
            (6.5, 8.5, ["q", "w"], "mounted_qw"),
            (9.0, 11.0, ["e", "w"], "mounted_ew"),
            (11.5, 13.5, ["w"], "mounted_w2"),
            (14.0, 16.0, ["a", "w"], "mounted_turn"),
            (16.5, 18.5, [], "mounted_stop"),
            (19.0, 19.3, ["w"], "mounted_prejump"),
            (20.0, 20.3, ["space"], "mounted_jump"),
            (21.0, 22.0, [], "mounted_afterjump"),
        ]
        names = {}

        def run(until, phase):
            last_state = 0.0
            while time.monotonic() < until:
                row = sample(mem, calls, models, names)
                row["phase"] = phase
                if time.monotonic() - last_state > 0.2:
                    try:
                        row["state"] = client.lua(STATE_LUA)
                    except Exception as e:
                        row["state"] = "error: %s" % str(e)[:80]
                    last_state = time.monotonic()
                result["samples"].append(row)
                time.sleep(args.period)

        t0 = time.monotonic()
        for at, until, keys, label in phases:
            delay = t0 + at - time.monotonic()
            if delay > 0:
                time.sleep(delay)
            if keys:
                client.release()
                duration = max(0.05, until - at) * 1000.0
                for key in keys:
                    client.key(key, duration)
            run(t0 + until, label)
        client.release()

        run_calls(mem, calls, models, result)
        Path(args.out).mkdir(parents=True, exist_ok=True)
        (Path(args.out) / "anim_d.json").write_text(json.dumps(result), encoding="utf-8")
        print("wrote", Path(args.out) / "anim_d.json", "samples", len(result["samples"]),
              "calls", len(result.get("calls", [])))
    finally:
        try:
            client.release()
        except Exception:
            pass
        if client.pid:
            try:
                d.text("client_stop", pid=client.pid)
            except Exception:
                pass
        d.close()


if __name__ == "__main__":
    main()
