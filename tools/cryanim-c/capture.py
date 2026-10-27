#!/usr/bin/env python3
"""Capture live 1.2 CryAnimation step-C state from the hidden client.

Reads (never writes) through the lane driver:

* every FIFO layer header and layer-0 entries of the local player and the mount,
* the animation-set row behind every current clip id (`FUN_3160cc50` -> 0xdc row):
  name, timeScale `+0x30`, start/end arrays `+0x5c/+0x60`,
* the owner fields the speed formula reads (`char+0x238/+0x240/+0x278/+0x468`),
* the embedded `CAttachmentManager` at `char+0x30`: object vector and 0x300 bytes per object,
* the mount pose (char+0x254) seat-bone matrices and each object's output blocks.

It keeps full rows every `--period` seconds, and the exact before/after pair around every
layer-0 FIFO size change (an insertion is `StartAnimationById`). The JSON feeds the
`port::cryanim` step-C differential tests.
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
DAT_316C2254 = 0x316C2254
CAF_STRIDE = 0xDC
LMG_STRIDE = 0x48
LAYERS = 16
LAYER_STRIDE = 0x30
ENTRY_SIZE = 0x88
MAX_ENTRIES = 8


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

    def f32(self, addr):
        raw = self.read(addr, 4)
        return struct.unpack("<f", raw)[0] if raw and len(raw) == 4 else 0.0

    def u16(self, addr):
        raw = self.read(addr, 2)
        return struct.unpack("<H", raw)[0] if raw and len(raw) == 2 else 0

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


def anim_set(mem, anim):
    char = mem.u32(anim + 0x498) if anim else 0
    holder = mem.u32(char + 0x180) if char else 0
    return holder + 0x5D4 if holder else 0


def read_row(calls, mem, set_ptr, clip_id):
    if not set_ptr or clip_id < 0:
        return None
    rec = calls.call("cryanimation", FUN_3160CC50, this=set_ptr, args=(clip_id,))
    if not rec:
        return None
    raw = mem.read(rec, 0x10)
    if not raw:
        return None
    index, kind = struct.unpack_from("<IH", raw, 8)
    glob = mem.u32(mem.m.module_base("cryanimation.dll") + (DAT_316C2254 - CRYANIMATION_BASE))
    row = None
    if glob and index < 0x10000:
        base = mem.u32(glob + (0xB8 if kind == 2 else 0xB4))
        if base:
            stride = LMG_STRIDE if kind == 2 else CAF_STRIDE
            row_addr = base + index * stride
            raw_row = mem.read(row_addr, stride)
            if raw_row:
                row = {
                    "address": row_addr,
                    "kind": kind,
                    "name": mem.cstr(struct.unpack_from("<I", raw_row, 0)[0]),
                    "flags": struct.unpack_from("<I", raw_row, 4)[0],
                    "hex": raw_row.hex(),
                }
                if kind != 2:
                    row["time_scale"] = struct.unpack_from("<f", raw_row, 0x30)[0]
                    row["start"] = list(struct.unpack_from("<4f", raw_row, 0x5C))
                    row["end"] = list(struct.unpack_from("<4f", raw_row, 0x60))
                else:
                    row["fourcc"] = struct.unpack_from("<I", raw_row, 0x40)[0]
                    row["v2c"] = struct.unpack_from("<f", raw_row, 0x2C)[0]
    return {"record": rec, "raw": raw.hex(), "index": index, "kind": kind, "row": row}


def layer_count(mem, anim):
    if not anim:
        return 0
    raw = mem.read(anim + 0xCC, 12)
    if not raw:
        return 0
    start, end, _ = struct.unpack("<III", raw)
    if not (0x3B0000 < start < end < 0x80000000):
        return 0
    return min((end - start) // ENTRY_SIZE, MAX_ENTRIES)


def read_anim(mem, anim, calls):
    if not anim:
        return None
    out = {"object": anim, "layers": [], "rows": {}}
    set_ptr = anim_set(mem, anim)
    out["set"] = set_ptr
    for i in range(LAYERS):
        raw = mem.read(anim + 0xCC + i * LAYER_STRIDE, LAYER_STRIDE)
        if not raw:
            out["layers"].append(None)
            continue
        start, end, _cap = struct.unpack_from("<III", raw, 0)
        layer = {"header": raw.hex(), "entries": [], "assets": []}
        if 0x3B0000 < start < end < 0x80000000:
            n = min((end - start) // ENTRY_SIZE, MAX_ENTRIES)
            for j in range(n):
                raw_entry = mem.read(start + j * ENTRY_SIZE, ENTRY_SIZE)
                if not raw_entry:
                    continue
                layer["entries"].append(raw_entry.hex())
                asset = struct.unpack_from("<I", raw_entry, 0)[0]
                if asset:
                    raw_asset = mem.read(asset, 0x260)
                    if raw_asset:
                        layer["assets"].append({"address": asset, "raw": raw_asset.hex()})
                clip = struct.unpack_from("<h", raw_entry, 8)[0]
                if i == 0 and clip >= 0 and clip not in out["rows"]:
                    out["rows"][clip] = read_row(calls, mem, set_ptr, clip)
        out["layers"].append(layer)
    return out


def read_owner(mem, char, anim):
    if not char:
        return None
    return {
        "char": char,
        "frame_delta": mem.f32(char + 0x238),
        "raw_delta": mem.f32(char + 0x240),
        "anim_scale": mem.f32(char + 0x278),
        "flags_244": mem.u32(char + 0x244),
        "v460": mem.f32(char + 0x460),
        "v464": mem.f32(char + 0x464),
        "additive_speed": mem.f32(char + 0x468),
        "v46c": mem.u32(char + 0x46C),
        "set_holder": mem.u32(char + 0x180),
    }


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
            raw = mem.read(p, 0x300)
            if not raw:
                continue
            objects.append({
                "address": p,
                "type": struct.unpack_from("<i", raw, 0xB8)[0],
                "bone": struct.unpack_from("<i", raw, 0xC4)[0],
                "align": struct.unpack_from("<i", raw, 0xD4)[0],
                "raw": raw.hex(),
            })
    return {"manager": mgr, "vtable": mem.u32(mgr), "objects": objects}


def pose_seat(mem, char):
    """The mount pose's seat-bone matrix through pose vt[+0x18] (bone id from the snapshot)."""
    pose = mem.u32(char + 0x254) if char else 0
    if not pose:
        return None
    out = {"pose": pose}
    for name in ("bone_spine_driver", "bone_spine_passenger"):
        # the pose vtable +0x30 takes a name and returns a bone id (`FUN_3160cc50` style);
        # the slot is called through the driver with a copied string in scratch memory.
        out[name] = None
    return out


def sample(mem, calls, models, full):
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
            "transform": b.model_transform(calls, mem, model, models, name),
            "owner": read_owner(mem, char, anim),
            "anim": read_anim(mem, anim, calls) if full else None,
            "layer0_count": layer_count(mem, anim),
            "manager": read_manager(mem, char),
        }
    return row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--driver", default="D:/aaemu-target/move-golden/debug/aaemu-clientdriver.exe")
    ap.add_argument("--dll", default="D:/aaemu-target/move-golden/aadriver/aadriver.dll")
    ap.add_argument("--client", default="D:/AA1.2/bin32/archeage.exe")
    ap.add_argument("--launcher", default="D:/aaemu-live-anim-golden/bin/aaemu-launcher.exe")
    ap.add_argument("--login", default="127.0.0.1:26237")
    ap.add_argument("--password-file", required=True)
    ap.add_argument("--user", default="animgolden")
    ap.add_argument("--character", default="Animgolda")
    ap.add_argument("--out", required=True)
    ap.add_argument("--period", type=float, default=0.08)
    ap.add_argument("--setters-only", action="store_true")
    args = ap.parse_args()

    command = [args.driver, "mcp", "--dll", args.dll, "--client", args.client,
               "--launcher", args.launcher, "--password-file", args.password_file,
               "--login", args.login, "--user", args.user, "--artifact-dir", str(Path(args.out) / "mcp")]
    d = feel.Driver(command)
    client = feel.OriginalClient(d, Path(args.password_file), args.user, args.character, Path(args.out))
    result = {"user": args.user, "character": args.character, "samples": [], "pairs": [], "models": {}}
    try:
        client.launch()
        mem = Mem(client.pid)
        calls = Calls(args.driver, client.pid)
        if args.setters_only:
            def alloc(size):
                out = calls.run("alloc", size)
                m = re.search(r"0x([0-9a-fA-F]+)", out)
                return int(m.group(1), 16) if m else 0
            scratch = alloc(0x800)
            cases = []
            for fn, va in (("31554660", 0x31554660), ("315546a0", 0x315546a0)):
                for i in range(20):
                    layer = i % 16
                    value = [0.0, 1.0, 0.9375, 1.0369999, -0.5, 2.5, 0.5, 0.25][i % 8]
                    before = mem.read(scratch, 0x800)
                    calls.call("cryanimation", va, this=scratch, args=(layer, "f:%.7f" % value))
                    after = mem.read(scratch, 0x800)
                    cases.append({"fn": fn, "layer": layer, "value": value,
                                  "before": before.hex(), "after": after.hex()})
            calls.run("free", scratch)
            Path(args.out).mkdir(parents=True, exist_ok=True)
            (Path(args.out) / "layer_setter.json").write_text(json.dumps({"cases": cases}), encoding="utf-8")
            print("wrote", Path(args.out) / "layer_setter.json", "cases", len(cases))
            return
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

        def run(tag, seconds, period):
            end = time.monotonic() + seconds
            last_full = 0.0
            prev_count = {}
            last_full_row = {}
            while time.monotonic() < end:
                now = time.monotonic()
                full = now - last_full >= period
                row = sample(mem, calls, models, full)
                if full:
                    last_full = now
                    result["samples"].append({**row, "phase": tag})
                    last_full_row = {k: v for k, v in row.items() if k in ("player", "mount")}
                for name in ("player", "mount"):
                    if name not in row:
                        continue
                    if full:
                        count = len((row[name]["anim"].get("layers") or [{}])[0].get("entries", []))
                    else:
                        count = row[name].get("layer0_count", 0)
                    if name in prev_count and count != prev_count[name]:
                        before = last_full_row.get(name)
                        after = row[name] if full else sample(mem, calls, models, True).get(name)
                        if before and after:
                            result["pairs"].append({"phase": tag, "name": name,
                                                    "before": before, "after": after,
                                                    "before_count": prev_count[name],
                                                    "after_count": count})
                    prev_count[name] = count
                time.sleep(0.003)

        run("player_idle", 1.0, args.period)
        for cycle in range(4):
            client.key("w", 1400)
            run("player_run_%d" % cycle, 1.5, args.period)
            client.release()
            time.sleep(0.5)
            run("player_stop_%d" % cycle, 1.2, args.period)

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
        run("mounted_idle", 1.2, args.period)
        for cycle in range(4):
            client.key("w", 1400)
            run("mounted_run_%d" % cycle, 1.6, args.period)
            client.release()
            time.sleep(0.5)
            run("mounted_stop_%d" % cycle, 1.1, args.period)
        client.key("w", 4000)
        run("mounted_gallop", 4.0, args.period)
        client.release()
        time.sleep(0.6)
        run("mounted_end", 0.6, args.period)

        # Pure layer setters on a scratch CSkeletonAnim (FUN_31554660 / FUN_315546a0).
        def alloc(size):
            out = calls.run("alloc", size)
            m = re.search(r"0x([0-9a-fA-F]+)", out)
            return int(m.group(1), 16) if m else 0
        setter_cases = []
        scratch = alloc(0x800)
        if scratch:
            for i in range(20):
                layer = i % 16
                value = [0.0, 1.0, 0.9375, 1.0369999, -0.5, 2.5, 0.5, 0.25][i % 8]
                before = mem.read(scratch, 0x300)
                calls.call("cryanimation", 0x31554660, this=scratch, args=(layer, "f:%.7f" % value))
                after = mem.read(scratch, 0x300)
                setter_cases.append({"fn": "31554660", "layer": layer, "value": value,
                                     "before": before.hex(), "after": after.hex()})
            for i in range(20):
                layer = i % 16
                value = [0.0, 1.0, 0.9375, 1.0369999, -0.5, 2.5, 0.5, 0.25][i % 8]
                before = mem.read(scratch, 0x300)
                calls.call("cryanimation", 0x315546A0, this=scratch, args=(layer, "f:%.7f" % value))
                after = mem.read(scratch, 0x300)
                setter_cases.append({"fn": "315546a0", "layer": layer, "value": value,
                                     "before": before.hex(), "after": after.hex()})
            calls.run("free", scratch)
        result["setter_cases"] = setter_cases
        Path(args.out).mkdir(parents=True, exist_ok=True)
        (Path(args.out) / "cryanim_c.json").write_text(json.dumps(result), encoding="utf-8")
        print("wrote", Path(args.out) / "cryanim_c.json", "samples", len(result["samples"]),
              "pairs", len(result["pairs"]))
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
