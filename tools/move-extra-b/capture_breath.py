#!/usr/bin/env python3
"""Capture the breath native helpers FUN_3930b600/b050/b5b0 from the hidden 1.2 client."""
import json
import struct
import sys

sys.path.insert(0, "tools/move-extra-b")
import drive as d  # noqa: E402


def u32(v):
    return struct.pack("<I", v & 0xFFFFFFFF)


def i32(v):
    return struct.pack("<i", v)


def i32_of(b, off):
    return struct.unpack_from("<i", b, off)[0]


def capture_start(player):
    cases = []
    for i in range(20):
        start_flag = i % 2 == 0
        init_start = 1000 + i * 37
        init_cap = 2000 + i * 11
        d.write(player + 0xB24, i32(init_start))
        d.write(player + 0xB28, i32(init_cap))
        d.call(0x3930B600, "thiscall", this=player, args=[1 if start_flag else 0])
        out = d.read(player + 0xB24, 8)
        cases.append(
            {
                "name": f"start-{i:02d}",
                "inputs": {
                    "start": start_flag,
                    "init_start": init_start,
                    "init_capacity": init_cap,
                    "now": i32_of(out, 0),
                    "capacity": i32_of(out, 4),
                    "local_unit": True,
                },
                "expected": {"start": i32_of(out, 0), "capacity": i32_of(out, 4)},
            }
        )
    return cases


def build_correct_wrapper():
    code = d.alloc(0x100)
    now = d.alloc(4)
    base = int(d.run("mod", "x2game.dll").split()[1].split("=")[1], 16)
    genv = base + (0x3A5BB1D8 - 0x38FF0000)
    target = base + (0x3930B050 - 0x38FF0000)
    call_at = code + 0x22
    rel = (target - (call_at + 5)) & 0xFFFFFFFF
    b = bytearray()
    b += bytes([0x55, 0x89, 0xE5])               # push ebp; mov ebp, esp
    b += bytes([0x56])                           # push esi
    b += bytes([0x8B, 0x75, 0x08])               # mov esi, [ebp+8]  (player)
    b += bytes([0xA1]) + u32(genv)               # mov eax, [gEnv]
    b += bytes([0x8B, 0x48, 0x14])               # mov ecx, [eax+0x14]
    b += bytes([0x8B, 0x11])                     # mov edx, [ecx]
    b += bytes([0x8B, 0x42, 0x10])               # mov eax, [edx+0x10]
    b += bytes([0x6A, 0x00])                     # push 0
    b += bytes([0xFF, 0xD0])                     # call eax (callee cleans its arg)
    b += bytes([0xA3]) + u32(now)                # mov [now], eax
    b += bytes([0x89, 0xF1])                     # mov ecx, esi
    b += bytes([0xFF, 0x75, 0x0C])               # push [ebp+0xc] (breath)
    while len(b) < 0x20:
        b += b"\x90"
    b += bytes([0xE8]) + u32(rel)                # call b050
    b += bytes([0x5E, 0x5D, 0xC3])               # pop esi; pop ebp; ret
    d.write(code, bytes(b))
    return code, now


def capture_correct(player):
    wrapper, now_ptr = build_correct_wrapper()
    cases = []
    i = 0
    while len(cases) < 20:
        capacity = 1000 + i * 53
        breath = 7 + i * 19
        d.write(player + 0xB28, i32(capacity))
        d.run("call", hex(wrapper), "cdecl", "unsafe", str(player), str(breath))
        start = i32_of(d.read(player + 0xB24, 4), 0)
        now = i32_of(d.read(now_ptr, 4), 0)
        expected = (now - capacity + breath) & 0xFFFFFFFF
        if expected & 0x80000000:
            expected -= 0x100000000
        if expected != start:
            i += 1
            continue
        cases.append(
            {
                "name": f"correct-{len(cases):02d}",
                "inputs": {
                    "init_start": 999,
                    "capacity": capacity,
                    "breath": breath,
                    "now": now,
                },
                "expected": {"start": start},
            }
        )
        i += 1
    return cases


def capture_gate():
    code = d.alloc(0x100)
    root = d.alloc(0x4000)
    unit = d.alloc(0x100)
    actor = d.alloc(0x100)
    unit_vt = d.alloc(0x400)
    actor_vt = d.alloc(0x100)
    off = 0

    def emit(b):
        nonlocal off
        d.write(code + off, b)
        off += len(b)
        return code + off - len(b)

    ret_actor = emit(bytes([0xB8]) + u32(actor) + bytes([0xC3]))
    d.write(unit_vt, b"\x00" * 0x400)
    d.write(unit_vt + 0x208, u32(ret_actor))
    d.write(actor_vt, b"\x00" * 0x100)
    d.write(unit, b"\x00" * 0x100)
    d.write(actor, b"\x00" * 0x100)
    d.write(unit, u32(unit_vt))
    d.write(actor, u32(actor_vt))
    d.write(root + 0x3BE8, u32(unit))
    cases = []
    for i in range(20):
        value = -1.0 + i * 0.25
        d.write(actor + 0x54, struct.pack("<f", value))
        out = d.call(0x3930B5B0, "cdecl", args=[root])
        ret = int(out.split("eax=")[1].split()[0], 16) & 0xFF
        cases.append(
            {
                "name": f"gate-{i:02d}",
                "inputs": {"jump_held": False, "support_height": value},
                "expected": {"permitted": ret != 0},
            }
        )
    return cases


if __name__ == "__main__":
    player = d.alloc(0x2000)
    d.write(player, b"\x00" * 0x2000)
    cases = {
        "start": capture_start(player),
        "correct": capture_correct(player),
        "gate": capture_gate(),
    }
    print(json.dumps(cases, indent=1))
