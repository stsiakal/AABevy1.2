"""Execute the shipping CryEntitySystem GetViewDistance (0x3207DAC0) in Unicorn.

The function is virtual and needs a live render-proxy object; the audit calls it
CEntity::GetViewDistance (cryentitysystem.dll, image base 0x32000000). This oracle maps the
original image plus small stubs for its three calls - the cvar float getter, the entity-class
probe (vt+0x1DC) and the bbox getter (vt+0x6C) - and executes the original instructions, so the
golden values are the original's, not a reimplementation. The cvars are the live 1.2 values
(e_view_dist_min 0, e_view_dist_doodad_min 64, e_view_dist_ratio 60,
e_view_dist_ratio_detail 30, e_view_dist_custom_ratio 100).

    python tools/native_view_distance_oracle.py --dll D:/aa-re/client12/engine/cryentitysystem.dll \
        --out crates/aa-client/src/port/world/goldens/view_distance.json
"""
from __future__ import annotations

import argparse
import json
import struct

import pefile
from unicorn import UC_ARCH_X86, UC_MODE_32, Uc, UcError
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP

FUNC = 0x3207DAC0
RET_STUB = 0x50001000
GETTER = 0x50002000
CLASS_STUB = 0x50002100
BBOX_STUB = 0x50002200
STACK = 0x50010000
OUT = 0x50030000
OBJ = 0x50040000
CLASS_OBJ = 0x50041000
CLASS_VAL = 0x50042000
BBOX = 0x50043000
FVT = 0x50044000
CVT = 0x50045000
CVARS = 0x50046000
CVAR_VALUES = 0x50047000
CVAR_ADDR = {
    0x320ECB9C: 0,  # e_view_dist_min
    0x320ECB98: 1,  # e_view_dist_doodad_min
    0x320ECB94: 2,  # e_view_dist_ratio
    0x320ECB90: 3,  # e_view_dist_ratio_detail
    0x320ECB8C: 4,  # e_view_dist_custom_ratio
}
NOP6 = bytes(6)
NOP7 = bytes(7)
FS_READ = bytes.fromhex("64a100000000")
FS_WRITE25 = bytes.fromhex("648925")
FS_WRITE0D = bytes.fromhex("64890d")


def put(u, addr, data):
    u.mem_write(addr, data)


def f32(u, addr, values):
    put(u, addr, struct.pack("<%df" % len(values), *values))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dll", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    pe = pefile.PE(args.dll)
    base = pe.OPTIONAL_HEADER.ImageBase
    image = bytearray(pe.get_memory_mapped_image())
    # The SEH prologue/epilogue only needs to run; patch its fs: accesses out for the emulator.
    start = FUNC - base
    end = start + 0x420
    body = bytes(image[start:end]).replace(FS_READ, b"\x90" * 6)
    for op in (FS_WRITE25, FS_WRITE0D):
        while True:
            at = body.find(op)
            if at < 0:
                break
            body = body[:at] + b"\x90" * 7 + body[at + 7:]
    image[start:end] = body

    u = Uc(UC_ARCH_X86, UC_MODE_32)
    u.mem_map(base, (pe.OPTIONAL_HEADER.SizeOfImage + 0xFFF) & ~0xFFF)
    put(u, base, bytes(image))
    for page in (0x50000000, 0x50010000, 0x50020000, 0x50030000, 0x50040000):
        u.mem_map(page, 0x10000)

    # `fstp dword [OUT]` returns the x87 result to the controller.
    put(u, RET_STUB, b"\xd9\x1d" + struct.pack("<I", OUT))
    # cvar GetFVal: `mov edx,[ecx+4]; fld [edx]; ret`
    put(u, GETTER, b"\x8b\x51\x04\xd9\x02\xc3")
    # class probe: returns a pointer whose first dword is 0 (unit) or 1 (doodad); pops 8.
    put(u, CLASS_STUB, b"\xb8" + struct.pack("<I", CLASS_VAL) + b"\xc2\x08\x00")
    # bbox getter: returns BBOX; pops 4.
    put(u, BBOX_STUB, b"\xb8" + struct.pack("<I", BBOX) + b"\xc2\x04\x00")
    put(u, FVT, bytes(0x200))
    put(u, FVT + 0x6C, struct.pack("<I", BBOX_STUB))
    put(u, FVT + 0x1DC, struct.pack("<I", CLASS_STUB))
    put(u, CVT, bytes(0x40))
    put(u, CVT + 0xC, struct.pack("<I", GETTER))
    for i in range(5):
        addr = CVARS + i * 0x10
        put(u, addr, struct.pack("<II", CVT, CVAR_VALUES + i * 4))
    # Already-initialised cvar pointers: skip the console lookup path.
    put(u, 0x320ECBA0, struct.pack("<I", 0x1F))
    for va, i in CVAR_ADDR.items():
        put(u, va, struct.pack("<I", CVARS + i * 0x10))

    cvars = [0.0, 64.0, 60.0, 30.0, 100.0]
    f32(u, CVAR_VALUES, cvars)

    cases = [
        (30, 0x000, False, False, (-0.5, -0.5, 0.0), (0.5, 0.5, 1.8)),
        (30, 0x000, False, True, (-0.5, -0.5, 0.0), (0.5, 0.5, 1.8)),
        (30, 0x400, True, False, (-0.5, -0.5, 0.0), (0.5, 0.5, 1.8)),
        (100, 0x400, True, False, (-1.0, -0.5, 0.0), (1.0, 0.5, 3.0)),
        (255, 0x000, False, False, (-1.0, -1.0, -1.0), (1.0, 1.0, 1.0)),
        (255, 0x400, False, False, (-2.0, -1.0, -0.5), (2.0, 1.0, 4.0)),
        (0, 0x000, False, False, (-1.0, -1.0, -1.0), (1.0, 1.0, 1.0)),
        (1, 0x000, True, False, (-20.0, -10.0, 0.0), (20.0, 10.0, 5.0)),
        (45, 0x000, False, True, (-0.3, -0.2, 0.0), (0.3, 0.2, 0.6)),
        (60, 0x400, False, False, (-0.2, -0.2, 0.0), (0.2, 0.2, 0.4)),
        (90, 0x400, True, False, (-3.0, -2.0, 0.0), (3.0, 2.0, 6.0)),
        (30, 0x000, True, True, (-1.5, -1.0, 0.0), (1.5, 1.0, 2.0)),
        (7, 0x000, False, False, (-0.4, -0.4, 0.0), (0.4, 0.4, 1.7)),
        (200, 0x400, True, False, (-1.0, -1.0, 0.0), (1.0, 1.0, 1.0)),
        (100, 0x000, False, True, (-10.0, -10.0, -10.0), (10.0, 10.0, 10.0)),
        (100, 0x000, False, True, (10.0, -1.0, -1.0), (-10.0, 1.0, 1.0)),  # inverted x: 0
        (255, 0x400, True, False, (0.0, 0.0, 0.0), (0.5, 0.5, 0.5)),
        (3, 0x400, False, False, (-50.0, -1.0, 0.0), (50.0, 1.0, 2.0)),
        (30, 0x000, True, False, (-0.1, -0.1, 0.0), (0.1, 0.1, 0.2)),
        (180, 0x000, True, True, (-0.75, -0.5, 0.0), (0.75, 0.5, 1.5)),
    ]
    rows = []
    for ratio, flags, doodad, detail, lo, hi in cases:
        u.mem_write(OBJ, bytes(0x100))
        u.mem_write(CLASS_OBJ, bytes(0x20))
        put(u, OBJ + 0, struct.pack("<I", FVT))
        put(u, OBJ + 0xC, struct.pack("<I", BBOX))
        put(u, OBJ + 0x14, bytes([ratio]))
        put(u, OBJ + 0x0F, bytes([7 if detail else 0]))
        put(u, OBJ + 0x48, struct.pack("<I", CLASS_OBJ))
        put(u, OBJ + 0x94, struct.pack("<I", flags))
        f32(u, OBJ + 0xA8, [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]])
        put(u, CLASS_OBJ, struct.pack("<I", FVT))
        f32(u, BBOX, [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]])
        put(u, CLASS_VAL, struct.pack("<I", 1 if doodad else 0))
        put(u, STACK, bytes(0x400))
        put(u, STACK, struct.pack("<I", RET_STUB))
        u.reg_write(UC_X86_REG_ECX, OBJ)
        u.reg_write(UC_X86_REG_ESP, STACK)
        try:
            u.emu_start(FUNC, RET_STUB + 6, count=20000)
        except UcError as e:
            pc = u.reg_read(UC_X86_REG_EIP)
            raise SystemExit(f"unicorn {e} at {pc:#x} (case {len(rows)})") from e
        (value,) = struct.unpack("<f", u.mem_read(OUT, 4))
        rows.append({
            "ratio": ratio, "flags": flags, "doodad": doodad, "detail": detail,
            "min": list(lo), "max": list(hi), "view_distance": value,
        })

    golden = {
        "source": "CryEntitySystem.dll GetViewDistance 0x3207DAC0 executed in Unicorn "
                  "(image base 0x32000000, shipped cryentitysystem.dll)",
        "cvars": dict(zip(["e_view_dist_min", "e_view_dist_doodad_min", "e_view_dist_ratio",
                           "e_view_dist_ratio_detail", "e_view_dist_custom_ratio"], cvars)),
        "cases": rows,
    }
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(golden, f, indent=2)
        f.write("\n")
    print(f"wrote {len(rows)} cases to {args.out}")


if __name__ == "__main__":
    main()
