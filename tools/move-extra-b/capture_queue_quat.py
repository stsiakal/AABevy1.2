#!/usr/bin/env python3
"""Capture FUN_390a58b0's queue submission (quaternion + flags) from the hidden 1.2 client."""
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


def matrix_from(yaw, pitch, roll, s0, s1, s2, tx, ty, tz):
    cy, sy = math.cos(yaw), math.sin(yaw)
    cp, sp = math.cos(pitch), math.sin(pitch)
    cr, sr = math.cos(roll), math.sin(roll)
    # Rz * Ry * Rx, rows
    r = [
        [cy * cp, cy * sp * sr - sy * cr, cy * sp * cr + sy * sr],
        [sy * cp, sy * sp * sr + cy * cr, sy * sp * cr - cy * sr],
        [-sp, cp * sr, cp * cr],
    ]
    return [
        r[0][0] * s0, r[0][1] * s0, r[0][2] * s0, tx,
        r[1][0] * s1, r[1][1] * s1, r[1][2] * s1, ty,
        r[2][0] * s2, r[2][1] * s2, r[2][2] * s2, tz,
    ]


CASES = [
    matrix_from(0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 2.0, 3.0),
    matrix_from(0.7, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(0.0, 0.4, 0.0, 1.0, 1.0, 1.0, 5.0, 6.0, 7.0),
    matrix_from(0.0, 0.0, 0.9, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(1.2, -0.3, 0.5, 1.0, 1.0, 1.0, -2.0, 0.0, 4.0),
    matrix_from(-2.0, 0.8, -0.2, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(0.3, 0.2, 0.1, 2.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(0.3, 0.2, 0.1, 1.0, 2.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(0.3, 0.2, 0.1, 1.0, 1.0, 2.0, 0.0, 0.0, 0.0),
    matrix_from(0.3, 0.2, 0.1, 0.5, 1.0, 3.0, 0.0, 0.0, 0.0),
    matrix_from(2.5, 1.1, -1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(-0.4, -0.9, 0.6, 1.0, 1.0, 1.0, 10.0, 20.0, 30.0),
    matrix_from(0.0, 1.4, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(0.0, -1.4, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(3.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(0.1, 0.1, 0.1, 0.01, 1.0, 1.0, 0.0, 0.0, 0.0),
    matrix_from(0.1, 0.1, 0.1, 1.0, 1.0, 0.01, 0.0, 0.0, 0.0),
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0],
]


def main():
    code = d.alloc(0x100)
    queue = d.alloc(0x20)
    vtable = d.alloc(0x40)
    matrix = d.alloc(0x40)
    rec = d.alloc(0x20)
    b = bytearray()
    b += bytes([0x8B, 0x44, 0x24, 0x04])          # mov eax, [esp+4] (quat *)
    for off in range(4):
        b += bytes([0x8B, 0x48, off * 4])        # mov ecx, [eax+off]
        b += bytes([0x89, 0x0D]) + u32(rec + off * 4)
    b += bytes([0x8B, 0x44, 0x24, 0x08])          # mov eax, [esp+8] (flags)
    b += bytes([0xA3]) + u32(rec + 16)
    b += bytes([0xC2, 0x08, 0x00])                # ret 8
    d.write(code, bytes(b))
    d.write(vtable, b"\x00" * 0x40)
    d.write(vtable + 0xC, u32(code))
    d.write(queue, b"\x00" * 0x20)
    d.write(queue, u32(vtable))
    cases = []
    for i, m in enumerate(CASES):
        param2 = i % 2
        param4 = [0x400000, 0x20000, 0, 0x400000][i % 4]
        d.write(matrix, b"".join(f32(v) for v in m))
        d.write(rec, b"\x00" * 0x20)
        d.call(
            0x390A58B0,
            "thiscall",
            this=queue,
            args=[str(param2), f"p:{hex(matrix)}", str(param4)],
        )
        quat = list(struct.unpack_from("<ffff", d.read(rec, 16)))
        flags = struct.unpack_from("<I", d.read(rec + 16, 4))[0]
        cases.append(
            {
                "name": f"queue-{i:02d}",
                "inputs": {"matrix": m, "param_2": param2, "param_4": param4},
                "expected": {"quat": quat, "flags": flags},
            }
        )
    return cases


if __name__ == "__main__":
    print(json.dumps(main(), indent=1))
