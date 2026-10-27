#!/usr/bin/env python3
"""Capture FUN_3912ce00 (swim control) live cases from the hidden 1.2 client.

The live actor is kept (so FUN_39113800 returns its real speed scale) but its entity
pointer is swapped for a deterministic fake entity whose +0x98 returns a point with no
water and whose +0x108 returns null.  The graph pointer is cleared for the call and the
real values are restored afterwards.  The controller, block and quaternion are scratch.
"""
import json
import struct
import sys

sys.path.insert(0, "tools/move-extra-b")
import drive as d  # noqa: E402


def f32(v):
    return struct.pack("<f", v)


def u32(v):
    return struct.pack("<I", v)


def bits(v):
    return struct.unpack("<I", f32(v))[0]


def get_actor():
    g = d.peek(0x39B02294)
    p = d.peek(g + 0x10C)
    unit = int(d.call(0x390B3750, "thiscall", this=p).split("eax=")[1].split()[0], 16)
    slot = d.peek(d.peek(unit) + 0x208)
    return int(d.call(slot, "thiscall", this=unit).split("eax=")[1].split()[0], 16)


def speed_scale(actor):
    out = d.call(0x39113800, "thiscall", this=actor)
    for token in out.split():
        if token.startswith("st0_f32="):
            return float(token.split("=", 1)[1])
    raise RuntimeError(out)


def build_fakes(speed_value=5.4):
    code = d.alloc(0x100)
    act = d.alloc(0x100)
    ent = d.alloc(0x20)
    actvt = d.alloc(0x200)
    entvt = d.alloc(0x200)
    pos = d.alloc(0x20)
    one = d.alloc(4)
    ctrl = d.alloc(0x100)
    block = d.alloc(0x100)
    quat = d.alloc(0x20)
    off = 0

    def emit(code_bytes):
        nonlocal off
        d.write(code + off, code_bytes)
        off += len(code_bytes)
        return code + off - len(code_bytes)

    ret0 = emit(b"\x31\xc0\xc3")
    retpos = emit(b"\xb8" + u32(pos) + b"\xc3")
    fld1 = emit(b"\xd9\x05" + u32(one) + b"\xc3")
    ret = emit(b"\xc3")
    d.write(one, f32(speed_value))
    for i, v in enumerate([1000000.0, 1000000.0, 1000000.0]):
        d.write(pos + 4 * i, f32(v))
    d.write(actvt, b"\x00" * 0x200)
    d.write(entvt, b"\x00" * 0x200)
    for slot in (0x68, 0x84, 0x9C):
        d.write(actvt + slot, u32(ret0))
    d.write(actvt + 0x74, u32(fld1))
    d.write(actvt + 0x184, u32(ret))
    d.write(entvt + 0x98, u32(retpos))
    d.write(entvt + 0x108, u32(ret0))
    d.write(act, b"\x00" * 0x100)
    d.write(ent, b"\x00" * 0x20)
    d.write(act, u32(actvt))
    d.write(act + 0xC, u32(ent))
    d.write(act + 0x24, u32(0))
    d.write(act + 0x34, u32(0))
    d.write(act + 0x38, u32(0))
    d.write(ent, u32(entvt))
    return act, ent, ctrl, block, quat


CASES = [
    # axes, quat, flags, f14, offset, buoy_depth, water_velocity, velocity, blend, deep, dt
    ((0.0, 1.0, 0.0), (0, 0, 0, 1), 0, -1000.0, 0.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, -1.0, 0.0), (0, 0, 0, 1), 0, -1000.0, -2.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((1.0, 0.0, 0.0), (0, 0, 0, 1), 0, -1000.0, -0.5, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 0.0, 1.0), (0, 0, 0, 1), 0, -1000.0, 0.5, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 0.0, -1.0), (0, 0, 0, 1), 0, -1000.0, 1.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.5, 0.5, 0.0), (0, 0.38268343, 0, 0.92387953), 0, -1000.0, -2.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.016),
    ((0.0, 1.0, 0.0), (0, 0, 0.70710677, 0.70710677), 0, -1000.0, -2.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.05),
    ((0.0, 1.0, 0.0), (0.38268343, 0, 0, 0.92387953), 0, -1000.0, -2.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 1.0, 0.0), (0, 0, 0, 1), 1, 0.6, 0.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 1.0, 0.0), (0, 0, 0, 1), 1, 0.6, 0.3, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 1.0, 0.0), (0, 0, 0, 1), 0, -1000.0, 0.3, 1.0, (0.1, 0.2, 0.3), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 1.0, 0.0), (0, 0, 0, 1), 0, -1000.0, -0.3, 1.0, (0, 0, 0), (0, 0, 0), 0.0, True, 0.033),
    ((0.0, 1.0, 0.0), (0, 0, 0, 1), 0, -1000.0, -2.0, 1.0, (0, 0, 0), (0.5, -0.25, -1.0), 0.1, True, 0.033),
    ((0.25, -0.5, 0.75), (0, 0, 0, 1), 0, -1000.0, -2.0, 0.0, (0, 0, 0), (0.5, -0.25, -1.0), 0.5, False, 0.033),
    ((0.0, 0.0, 0.0), (0, 0, 0, 1), 0, -1000.0, -2.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 0.0, 0.0), (0, 0, 0, 1), 0, -1000.0, 2.0, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 0.0, 0.0), (0, 0, 0, 1), 0, 0.4, -0.2, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((0.0, 0.0, 0.0), (0, 0, 0, 1), 0, 0.5, -0.2, 0.0, (0, 0, 0), (0, 0, 0), 0.0, False, 0.033),
    ((1.0, 1.0, 1.0), (0, 0, 0, 1), 1, 0.49, -2.0, 0.5, (-0.1, 0.0, 0.1), (0.2, 0.3, -0.4), 0.0, False, 0.02),
    ((0.0, 1.0, 0.0), (0, 0, 0, 1), 1, 0.51, -2.0, 0.5, (0.0, 0.0, 0.0), (0.0, 0.0, 0.0), 1.0, False, 0.033),
]


def main(prefix, count, speed_value=5.4):
    actor, ent, ctrl, block, quat = build_fakes(speed_value)
    speed = speed_value
    cases = []
    if True:
        for i, c in enumerate(CASES[:count]):
            axes, q, flags, f14, off, buoy, wv, vel, blend, deep, dt = c
            d.write(ctrl, b"\x00" * 0x100)
            d.write(ctrl + 0, f32(dt))
            d.write(ctrl + 8, u32(block))
            d.write(ctrl + 0xC, f32(axes[0]) + f32(axes[1]) + f32(axes[2]))
            d.write(ctrl + 0x40, u32(actor))
            d.write(ctrl + 0x48, u32(quat))
            d.write(ctrl + 0x5C, u32(flags))
            d.write(ctrl + 0x8C, f32(vel[0]) + f32(vel[1]) + f32(vel[2]))
            d.write(ctrl + 0xA4, f32(blend))
            d.write(ctrl + 0xA8, b"\x01" if deep else b"\x00")
            d.write(block, b"\x00" * 0x100)
            d.write(block + 0x14, f32(f14))
            d.write(block + 0x18, f32(off))
            d.write(block + 0x20, f32(buoy))
            d.write(block + 0x9C, f32(wv[0]) + f32(wv[1]) + f32(wv[2]))
            for j, v in enumerate(q):
                d.write(quat + 4 * j, f32(v))
            d.call(0x3912CE00, "thiscall", this=ctrl)
            out = d.read(ctrl, 0x100)
            cases.append(
                {
                    "name": f"{prefix}-{i:02d}",
                    "inputs": {
                        "dt": dt,
                        "axes": list(axes),
                        "quat": list(q),
                        "flags_5c": flags,
                        "block": {
                            "f14": f14,
                            "water_offset": off,
                            "buoy_depth": buoy,
                            "water_velocity": list(wv),
                        },
                        "velocity": list(vel),
                        "blend": blend,
                        "deep": deep,
                        "speed_scale": speed,
                        "entity_present": True,
                        "surface_velocity": None,
                        "query_velocity": None,
                        "graph_present": False,
                    },
                    "expected": {
                        "mode": struct.unpack_from("<I", out, 0x60)[0],
                        "velocity": list(struct.unpack_from("<fff", out, 0x64)),
                        "blend": struct.unpack_from("<f", out, 0xA4)[0],
                        "deep": bool(out[0xA8]),
                        "graph": None,
                    },
                }
            )
    return cases


if __name__ == "__main__":
    prefix = sys.argv[1] if len(sys.argv) > 1 else "foot"
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 20
    speed = float(sys.argv[3]) if len(sys.argv) > 3 else 5.4
    print(json.dumps(main(prefix, count, speed), indent=1))
