"""Minimal PE VA -> section/bytes reader for the unpacked 1.2 DLLs."""

import struct
import sys


class PE:
    def __init__(self, path):
        self.path = path
        with open(path, "rb") as f:
            self.data = f.read()
        pe = struct.unpack_from("<I", self.data, 0x3C)[0]
        assert self.data[pe:pe + 4] == b"PE\0\0", path
        nsec = struct.unpack_from("<H", self.data, pe + 6)[0]
        opt = pe + 24
        self.image_base = struct.unpack_from("<I", self.data, opt + 28)[0]
        sec = opt + struct.unpack_from("<H", self.data, pe + 20)[0]
        self.sections = []
        for i in range(nsec):
            off = sec + i * 40
            name = self.data[off:off + 8].rstrip(b"\0").decode("ascii", "replace")
            vsize, va, rawsize, rawptr = struct.unpack_from("<IIII", self.data, off + 8)
            chars = struct.unpack_from("<I", self.data, off + 36)[0]
            self.sections.append((name, va, vsize, rawptr, rawsize, chars))

    def section(self, va):
        rva = va - self.image_base
        for s in self.sections:
            name, sva, vsize, rawptr, rawsize, chars = s
            if sva <= rva < sva + max(vsize, rawsize):
                return s
        return None

    def read(self, va, n=16):
        s = self.section(va)
        if not s:
            return None
        name, sva, vsize, rawptr, rawsize, chars = s
        off = rawptr + (va - self.image_base - sva)
        if off < 0 or off + n > len(self.data):
            return None
        return self.data[off:off + n]

    def value(self, va):
        b = self.read(va, 8)
        if not b or len(b) < 4:
            return None
        u32 = struct.unpack_from("<I", b, 0)[0]
        f32 = struct.unpack_from("<f", b, 0)[0]
        return {"u32": u32, "i32": struct.unpack_from("<i", b, 0)[0],
                "f32": f32, "hex": b[:8].hex()}


FILES = {
    "x2game": "D:/aa-re/client12/x2game_unpacked.dll",
    "cryaction": "D:/aa-re/client12/engine/cryaction.dll",
    "cryanimation": "D:/aa-re/client12/engine/cryanimation.dll",
    "cryentitysystem": "D:/aa-re/client12/engine/cryentitysystem.dll",
    "crygame": "D:/aa-re/client12/engine/crygame.dll",
    "cry3dengine": "D:/aa-re/client12/engine2/cry3dengine.dll",
    "cryaisystem": "D:/aa-re/client12/engine2/cryaisystem.dll",
    "cryinput": "D:/aa-re/client12/engine2/cryinput.dll",
    "crymovie": "D:/aa-re/client12/engine2/crymovie.dll",
    "crysoundsystem": "D:/aa-re/client12/engine2/crysoundsystem.dll",
    "x2ui": "D:/aa-re/client12/engine3/x2ui.dll",
    "x2common": "D:/aa-re/client12/engine3/x2common.dll",
    "xlcommon": "D:/aa-re/client12/engine3/xlcommon.dll",
    "crysystem": "D:/aa-re/client12/ida/crysystem.dll",
    "cryscriptsystem": "D:/aa-re/client12/ida/cryscriptsystem.dll",
}

_CACHE = {}


def get(mod):
    if mod not in _CACHE:
        _CACHE[mod] = PE(FILES[mod])
    return _CACHE[mod]


def lookup(va):
    hits = []
    for mod in FILES:
        try:
            pe = get(mod)
        except (OSError, AssertionError):
            continue
        s = pe.section(va)
        if s:
            hits.append((mod, s[0], pe.value(va)))
    return hits


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        va = int(arg, 16)
        print(hex(va), lookup(va))
