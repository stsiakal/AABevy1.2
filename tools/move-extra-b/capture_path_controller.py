#!/usr/bin/env python3
"""Capture FUN_390ca3d0/FUN_390b2af0 path-controller construction on the hidden client."""
import json
import struct
import sys

sys.path.insert(0, "tools/move-extra-b")
import drive as d  # noqa: E402


def u32(v):
    return struct.pack("<I", v & 0xFFFFFFFF)


def f32(v):
    return struct.pack("<f", v)


def read_controller(addr):
    b = d.read(addr, 0x1A0)
    return {
        "timer": struct.unpack_from("<i", b, 4)[0],
        "unit": struct.unpack_from("<I", b, 0x18)[0],
        "start": list(struct.unpack_from("<fff", b, 0x1C)),
        "f28": b[0x28],
        "ids": list(struct.unpack_from("<5I", b, 0x2C)),
        "f150": list(struct.unpack_from("<3I", b, 0x150)),
        "active": b[0x15C],
        "vector": list(struct.unpack_from("<3I", b, 0x160)),
        "tail": list(struct.unpack_from("<6I", b, 0x170)),
        "timer_188": struct.unpack_from("<i", b, 0x188)[0],
        "f18c": b[0x18C],
        "f194": b[0x194],
        "f198": struct.unpack_from("<I", b, 0x198)[0],
        "timer_19c": struct.unpack_from("<i", b, 0x19C)[0],
    }


CASES = [
    (1, None, [1.0, 2.0, 3.0], 5.4),
    (1, None, [-4.0, 8.0, 16.0], 1.0),
    (1, None, [0.0, 0.0, 0.0], 7.25),
    (1, 7, [10.0, 20.0, 30.0], 5.4),
    (1, 7, [-1.0, -2.0, -3.0], 2.0),
    (1, 3, [5.0, 6.0, 7.0], 5.4),
    (1, 0, [9.0, 9.0, 9.0], 0.0),
    (1, None, [100.0, -100.0, 50.0], 12.5),
    (0, None, [1.0, 2.0, 3.0], 5.4),
    (0, 7, [1.0, 2.0, 3.0], 5.4),
    (1, None, [0.5, 0.25, 0.125], 3.3),
    (1, None, [-0.5, -0.25, -0.125], 4.4),
    (1, 7, [1234.0, 5678.0, 90.0], 6.6),
    (1, 3, [0.0, 0.0, 1.0], 8.8),
    (1, None, [42.0, 42.0, 42.0], 1.1),
    (1, None, [-42.0, 42.0, -42.0], 9.9),
    (1, 7, [7.0, 8.0, 9.0], 0.5),
    (1, 7, [11.0, 12.0, 13.0], 15.0),
    (0, 3, [1.0, 1.0, 1.0], 2.0),
    (1, None, [3.0, 1.0, 4.0], 5.4),
]


def main():
    code = d.alloc(0x200)
    unit = d.alloc(0x100)
    actor = d.alloc(0x100)
    unit_vt = d.alloc(0x400)
    actor_vt = d.alloc(0x100)
    old = d.alloc(0x200)
    old_vt = d.alloc(0x40)
    point = d.alloc(0x10)
    speed_addr = d.alloc(4)
    off = 0

    def emit(b):
        nonlocal off
        d.write(code + off, b)
        off += len(b)
        return code + off - len(b)

    ret_actor = emit(bytes([0xB8]) + u32(actor) + bytes([0xC3]))
    ret_1 = emit(bytes([0xB8, 0x01, 0x00, 0x00, 0x00, 0xC3]))
    ret_0 = emit(bytes([0x31, 0xC0, 0xC3]))
    ret_0_arg = emit(bytes([0x31, 0xC0, 0xC2, 0x04, 0x00]))
    ret_7 = emit(bytes([0xB8, 0x07, 0x00, 0x00, 0x00, 0xC3]))
    ret_3 = emit(bytes([0xB8, 0x03, 0x00, 0x00, 0x00, 0xC3]))
    ret_void = emit(bytes([0xC2, 0x04, 0x00]))
    fld_speed = emit(bytes([0xD9, 0x05]) + u32(speed_addr) + bytes([0xC3]))
    d.write(unit_vt, b"\x00" * 0x400)
    d.write(unit_vt + 4, u32(ret_1))
    d.write(unit_vt + 0x20, u32(ret_0_arg))
    d.write(unit_vt + 0x98, u32(ret_void))
    d.write(unit_vt + 0x208, u32(ret_actor))
    d.write(actor_vt, b"\x00" * 0x100)
    d.write(actor_vt + 0x74, u32(fld_speed))
    d.write(old_vt, b"\x00" * 0x40)
    d.write(old_vt + 8, u32(ret_7))
    d.write(unit, b"\x00" * 0x100)
    d.write(actor, b"\x00" * 0x100)
    d.write(old, b"\x00" * 0x200)
    d.write(unit, u32(unit_vt))
    d.write(actor, u32(actor_vt))
    d.write(old, u32(old_vt))
    d.write(old + 0x160, b"\x00" * 12)
    d.write(unit + 0x64, u32(unit + 0x64))  # empty intrusive list
    cases = []
    for i, (is_player, old_type, pt, speed) in enumerate(CASES):
        d.write(unit_vt + 4, u32(ret_1 if is_player else ret_0))
        d.write(unit + 0x58, u32(old) if old_type is not None else u32(0))
        if old_type is not None:
            d.write(old_vt + 8, u32(ret_7 if old_type == 7 else ret_3 if old_type == 3 else ret_0))
        d.write(speed_addr, f32(speed))
        d.write(point, b"".join(f32(v) for v in pt))
        out = d.call(0x390CA3D0, "thiscall", this=unit, args=[f"p:{hex(point)}"])
        ret = int(out.split("eax=")[1].split()[0], 16)
        new = struct.unpack("<I", d.read(unit + 0x58, 4))[0]
        ctrl = read_controller(new) if ret and new else None
        cases.append(
            {
                "name": f"path-{i:02d}",
                "inputs": {
                    "unit_is_player": bool(is_player),
                    "old_type": old_type,
                    "start": pt,
                    "speed": speed,
                },
                "expected": {"returned": bool(ret), "reused": old_type == 7, "controller": ctrl},
            }
        )
    return cases


if __name__ == "__main__":
    print(json.dumps(main(), indent=1))
