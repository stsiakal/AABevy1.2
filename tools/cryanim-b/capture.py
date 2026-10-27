#!/usr/bin/env python3
"""Capture live 1.2 CryAnimation state before/after real frames from the hidden client.

The original is launched through the lane stack's MCP driver (no OS input, hidden window),
entered on the given character, optionally summoning and mounting item 8159, and driven with the
client's own `client_input` path. Between inputs the script samples, through ReadProcessMemory:

* every `CSkeletonAnim` layer header and its FIFO entries,
* the owner `CCharInstance` fields the update path reads (+0x238/+0x240/+0x244/+0x460/+0x468),
* the `CAttachmentManager` object vector (raw 0x100-byte records) and the actor transforms.

Pointers are recovered the way the engine does it: GameClient -> local ClientUnit -> model actor,
model->vt[0x1b8]->vt[0x168](0) = ICharacterInstance, and char+0x254 -> CSkeletonPose+0x9b4 =
CSkeletonAnim. The mate's unit comes from the mate map (FUN_3933f5e0) and FUN_39360020.

Output is JSON for the `port::cryanim` differential tests; nothing is written to the client.
"""
import argparse
import json
import re
import struct
import subprocess
import sys
import time
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parents[1]
sys.path.insert(0, str(ROOT / "tools" / "feel-recorder"))

import run as feel  # noqa: E402

X2_BASE = 0x38FF0000
GAMECLIENT = 0x39B02294
MATE_MAP = 0x3A168304
FUN_3933F5E0 = 0x3933F5E0
FUN_39360020 = 0x39360020
CRYANIMATION_BASE = 0x31500000
CSKELETON_ANIM_VTABLE = 0x316779DC
LAYERS = 16
LAYER_STRIDE = 0x30
ENTRY_SIZE = 0x88
MAX_ENTRIES = 8


class WalkMem:
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
        b = self.read(addr, 4)
        return struct.unpack("<f", b)[0] if b and len(b) == 4 else 0.0


class Calls:
    def __init__(self, exe, pid):
        self.exe = str(exe)
        self.pid = str(pid)

    def run(self, *args, timeout=60):
        cmd = [self.exe, *[str(a) for a in args], "--pid", self.pid]
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
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

    def vt(self, obj, slot, conv="thiscall", args=()):
        vt = self.mem.u32(obj)
        fn = self.mem.u32(vt + slot) if vt else 0
        if not fn:
            return 0
        return self.call("x2game", fn, conv, this=obj, args=args)

    mem = None


def resolve_char(calls, mem, model):
    if not model:
        return 0, 0
    iface = calls.call("x2game", mem.u32(mem.u32(model) + 0x1B8), "thiscall", this=model) if model else 0
    vt = mem.u32(iface) if iface else 0
    fn = mem.u32(vt + 0x168) if vt else 0
    char = calls.call("x2game", fn, "thiscall", this=iface, args=(0,)) if fn else 0
    anim = 0
    if char:
        pose = mem.u32(char + 0x254)
        if pose:
            anim = mem.u32(pose + 0x9B4)
    return char, anim


def find_anim_by_vtable(mem, char):
    # Fallback when the pose link is not populated yet: scan writable regions for the vtable.
    needle = struct.pack("<I", CSKELETON_ANIM_VTABLE)
    m = feel.ProcessMemory(mem.pid)
    for base, size in _regions(m):
        data = m.read(base, size)
        if not data:
            continue
        i = 0
        while True:
            i = data.find(needle, i)
            if i < 0:
                break
            obj = base + i
            if m.u32(obj + 0x498) == char:
                return obj
            i += 4
    return 0


def _regions(m):
    import ctypes
    from ctypes import wintypes

    class MBI(ctypes.Structure):
        _fields_ = [
            ("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
            ("AllocationProtect", wintypes.DWORD), ("RegionSize", ctypes.c_size_t),
            ("State", wintypes.DWORD), ("Protect", wintypes.DWORD), ("Type", wintypes.DWORD),
        ]
    addr = 0
    while addr < 0x80000000:
        v = MBI()
        n = m.kernel.VirtualQueryEx(m.handle, ctypes.c_void_p(addr), ctypes.byref(v), ctypes.sizeof(v))
        if not n:
            break
        if v.State == 0x1000 and (v.Protect & 0xFF) in (2, 4, 8, 0x20, 0x40, 0x80) and v.RegionSize < 64 * 1024 * 1024:
            yield (v.BaseAddress or 0), v.RegionSize
        addr = (v.BaseAddress or 0) + v.RegionSize


def read_anim(mem, anim):
    if not anim:
        return None
    out = {"object": anim, "layers": []}
    for i in range(LAYERS):
        hdr = anim + 0xCC + i * LAYER_STRIDE
        raw = mem.read(hdr, LAYER_STRIDE)
        if not raw:
            out["layers"].append(None)
            continue
        start, end, cap = struct.unpack_from("<III", raw, 0)
        layer = {"header": raw.hex(), "entries": []}
        if 0x3B0000 < start < end < 0x80000000:
            n = min((end - start) // ENTRY_SIZE, MAX_ENTRIES)
            layer["entries"] = [b.hex() for b in
                                (mem.read(start + j * ENTRY_SIZE, ENTRY_SIZE) for j in range(n)) if b]
        out["layers"].append(layer)
    return out


def read_owner(mem, char, anim):
    if not char:
        return None
    bits = mem.read(anim + 0x460, 2) if anim else None
    owner = {
        "char": char,
        "frame_delta": mem.f32(char + 0x238),
        "raw_delta": mem.f32(char + 0x240),
        "flags_244": mem.u32(char + 0x244),
        "additive_speed": mem.f32(char + 0x468),
        "layer_bits": bits.hex() if bits else "",
        "anim_flags": mem.read(anim + 0x458, 0x10).hex() if anim else "",
        "rider": mem.u32(anim + 0x4A0) if anim else 0,
        "process": mem.read(anim + 0x4A4, 4).hex() if anim else "",
        "locator": mem.read(anim + 0x64C, 0x1C).hex() if anim else "",
    }
    return owner


def read_manager(mem, char):
    if not char:
        return None
    # `CAttachmentManager` is embedded at char+0x30 (its vtable is the first dword); the object
    # vector sits at manager+0x08/+0x0c.
    mgr = char + 0x30
    start = mem.u32(mgr + 0x08)
    end = mem.u32(mgr + 0x0C)
    objects = []
    if 0x3B0000 < start <= end < 0x80000000 and 0 < end - start <= 0x100 * 4:
        for j in range((end - start) // 4):
            p = mem.u32(start + j * 4)
            if not (0x3B0000 < p < 0x80000000):
                continue
            raw = mem.read(p, 0x100)
            objects.append({"address": p, "raw": raw.hex() if raw else ""})
    return {"manager": mgr, "objects": objects}


def model_transform(calls, mem, model, models, name):
    if not model:
        return None
    key = name + "_tf"
    p = models.get(key, 0)
    if not p:
        fn = mem.u32(mem.u32(model) + 0x228)
        if not fn:
            return None
        p = calls.call("x2game", fn, "thiscall", this=model)
        if not p:
            return None
        models[key] = p
    raw = mem.read(p, 0x40)
    return raw.hex() if raw else None


def sample(mem, calls, models):
    row = {"t": time.time()}
    for name in ("player", "mount"):
        model = models.get(name)
        if not model:
            continue
        char, anim = models.get(name + "_char", 0), models.get(name + "_anim", 0)
        if not char:
            char, anim = resolve_char(calls, mem, model)
            models[name + "_char"], models[name + "_anim"] = char, anim
        if char and not mem.u32(char):
            models.pop(name + "_char", None)
            models.pop(name + "_anim", None)
            continue
        if anim and mem.u32(anim + 0x498) != char:
            char, anim = resolve_char(calls, mem, model)
            models[name + "_char"], models[name + "_anim"] = char, anim
        row[name] = {
            "model": model,
            "transform": model_transform(calls, mem, model, models, name),
            "owner": read_owner(mem, char, anim),
            "anim": read_anim(mem, anim),
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
    ap.add_argument("--mode", choices=["on_foot", "mounted", "both"], default="both")
    ap.add_argument("--seconds", type=float, default=6.0)
    ap.add_argument("--kid", type=int, default=0, help="scripted 'W' hold seconds (0 = sample only)")
    args = ap.parse_args()

    command = [args.driver, "mcp", "--dll", args.dll, "--client", args.client,
               "--launcher", args.launcher, "--password-file", args.password_file,
               "--login", args.login, "--user", args.user, "--artifact-dir", str(Path(args.out) / "mcp")]
    d = feel.Driver(command)
    client = feel.OriginalClient(d, Path(args.password_file), args.user, args.character, Path(args.out))
    mem = None
    calls = None
    result = {"user": args.user, "character": args.character, "samples": [], "models": {}}
    try:
        client.launch()
        mem = WalkMem(client.pid)
        calls = Calls(args.driver, client.pid)
        calls.mem = mem
        try:
            client.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass
        client.chat("/move 10512.0 15585.0 232.0")
        time.sleep(2.0)

        base = mem.m.module_base("x2game.dll")
        game = mem.u32(base + (GAMECLIENT - X2_BASE))
        unit = mem.u32(game + 0x10C) if game else 0
        player_model = mem.u32(unit + 0x3BE8) if unit else 0
        models = {"player": player_model}
        char, anim = resolve_char(calls, mem, player_model)
        if char and not anim:
            anim = find_anim_by_vtable(mem.m, char)
        models.update({"player_char": char, "player_anim": anim})

        def sampling(tag, seconds):
            end = time.monotonic() + seconds
            rows = []
            while time.monotonic() < end:
                try:
                    rows.append(sample(mem, calls, models))
                except Exception as e:
                    rows.append({"t": time.time(), "error": str(e)})
                time.sleep(0.002)
            result["samples"].extend([{**r, "phase": tag} for r in rows])
            result["models"] = {k: v for k, v in models.items() if isinstance(v, int)}

        sampling("on_foot_idle", 1.0)
        if args.mode in ("on_foot", "both"):
            client.key("w", args.kid * 1000 if args.kid else 4000)
            sampling("on_foot_run", args.kid or 4.0)
            client.release()
            time.sleep(0.6)
            sampling("on_foot_stop", 0.6)
        if args.mode in ("mounted", "both"):
            client.ensure_item(8159)
            client.summon(8159)
            mate = calls.call("x2game", FUN_3933F5E0, "fastcall", this=base + (MATE_MAP - X2_BASE))
            mate_uid = mem.u32(mate + 0x20) if mate else 0
            mate_unit = calls.call("x2game", FUN_39360020, "cdecl", args=("0x%08x" % mate_uid,)) if mate_uid else 0
            mount_model = mem.u32(mate_unit + 0x3BE8) if mate_unit else 0
            models["mount"] = mount_model
            mchar, manim = resolve_char(calls, mem, mount_model) if mount_model else (0, 0)
            if mchar and not manim:
                manim = find_anim_by_vtable(mem.m, mchar)
            models.update({"mount_char": mchar, "mount_anim": manim})
            client.mount()
            sampling("mounted_idle", 1.0)
            client.key("w", args.kid * 1000 if args.kid else 5000)
            sampling("mounted_gallop", args.kid or 5.0)
            client.release()
            time.sleep(0.6)
            sampling("mounted_stop", 0.6)
        Path(args.out).mkdir(parents=True, exist_ok=True)
        (Path(args.out) / "cryanim_b.json").write_text(json.dumps(result), encoding="utf-8")
        print("wrote", Path(args.out) / "cryanim_b.json", "samples", len(result["samples"]))
    finally:
        try:
            client.release()
        except Exception:
            pass
        d.close()


if __name__ == "__main__":
    main()
