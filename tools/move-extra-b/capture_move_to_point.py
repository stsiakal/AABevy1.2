#!/usr/bin/env python3
"""Capture FUN_393ba480's submitted 0x9C movement request from the hidden 1.2 client."""
import json
import math
import struct
import sys

sys.path.insert(0, "tools/move-extra-b")
import drive as d  # noqa: E402


def u32(v):
    return struct.pack("<I", v & 0xFFFFFFFF)


def f32(v):
    return struct.pack("<f", v)


def qy(angle):
    return [0.0, math.sin(angle / 2), 0.0, math.cos(angle / 2)]


CASES = [
    (qy(0.0), [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]),
    (qy(0.7), [1.0, 2.0, 3.0], [10.0, 0.0, 0.0]),
    (qy(math.pi / 2), [0.0, 0.0, 0.0], [0.0, 10.0, 0.0]),
    (qy(-math.pi / 2), [5.0, 5.0, 5.0], [5.0, 15.0, 5.0]),
    (qy(math.pi), [-2.0, 0.0, 1.0], [8.0, 0.0, 1.0]),
    (qy(0.3), [0.0, 0.0, 0.0], [-10.0, 5.0, 0.0]),
    (qy(1.1), [100.0, -50.0, 20.0], [90.0, -40.0, 22.0]),
    (qy(-2.2), [-100.0, 20.0, -5.0], [-90.0, 25.0, -5.0]),
    ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.00001, 0.0, 0.0]),
    ([0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 1.0]),
    ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
    ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [-1.0, -1.0, -1.0]),
    ([0.38268343, 0.0, 0.0, 0.9238795], [2.0, 3.0, 4.0], [12.0, 3.0, 4.0]),
    ([0.0, 0.38268343, 0.0, 0.9238795], [2.0, 3.0, 4.0], [2.0, 13.0, 4.0]),
    ([0.0, 0.0, 0.38268343, 0.9238795], [2.0, 3.0, 4.0], [2.0, 3.0, 14.0]),
    ([-0.2, 0.3, -0.1, 0.92736185], [1.0, -2.0, 0.5], [7.0, 8.0, 9.0]),
    ([0.0, 0.70710677, 0.0, 0.70710677], [0.0, 0.0, 0.0], [3.0, -4.0, 0.0]),
    ([0.0, 0.0, 0.0, 1.0], [10.0, 10.0, 10.0], [0.0, 20.0, 0.0]),
    ([0.1, 0.2, 0.3, 0.92736185], [-5.0, -5.0, -5.0], [15.0, 5.0, 5.0]),
    ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1000.0, -1000.0, 0.0]),
]


def main():
    code = d.alloc(0x100)
    root = d.alloc(0x4000)
    unit = d.alloc(0x100)
    actor = d.alloc(0x100)
    movement = d.alloc(0x100)
    unit_vt = d.alloc(0x400)
    actor_vt = d.alloc(0x100)
    movement_vt = d.alloc(0x40)
    camera = d.alloc(0x40)
    point = d.alloc(0x10)
    rec = d.alloc(0x100)
    off = 0

    def emit(b):
        nonlocal off
        d.write(code + off, b)
        off += len(b)
        return code + off - len(b)

    ret_actor = emit(bytes([0xB8]) + u32(actor) + bytes([0xC3]))
    ret_movement = emit(bytes([0xB8]) + u32(movement) + bytes([0xC3]))
    ret_camera = emit(bytes([0xB8]) + u32(camera) + bytes([0xC3]))
    # movement->vtable[0]: copy the 0x9C request to rec
    rec_stub = emit(
        bytes([0x8B, 0x74, 0x24, 0x04, 0xBF]) + u32(rec)
        + bytes([0xB9, 0x27, 0x00, 0x00, 0x00, 0xF3, 0xA5, 0xC2, 0x04, 0x00])
    )
    d.write(unit_vt, b"\x00" * 0x400)
    d.write(unit_vt + 0x208, u32(ret_actor))
    d.write(unit_vt + 0x228, u32(ret_camera))
    d.write(actor_vt, b"\x00" * 0x100)
    d.write(actor_vt + 0xB0, u32(ret_movement))
    d.write(movement_vt, b"\x00" * 0x40)
    d.write(movement_vt, u32(rec_stub))
    d.write(unit, b"\x00" * 0x100)
    d.write(actor, b"\x00" * 0x100)
    d.write(movement, b"\x00" * 0x100)
    d.write(camera, b"\x00" * 0x40)
    d.write(unit, u32(unit_vt))
    d.write(actor, u32(actor_vt))
    d.write(movement, u32(movement_vt))
    d.write(root + 0x3BE8, u32(unit))
    cases = []
    for i, (quat, pos, pt) in enumerate(CASES):
        d.write(camera, b"".join(f32(v) for v in quat))
        d.write(camera + 0x10, b"".join(f32(v) for v in pos))
        d.write(point, b"".join(f32(v) for v in pt))
        d.write(rec, b"\x00" * 0x100)
        d.call(0x393BA480, "cdecl", args=[hex(root), f"p:{hex(point)}"])
        req = d.read(rec, 0x9C)
        cases.append(
            {
                "name": f"point-{i:02d}",
                "inputs": {"quat": quat, "pos": pos, "point": pt},
                "request": req.hex(),
            }
        )
    return cases


if __name__ == "__main__":
    print(json.dumps(main(), indent=1))
