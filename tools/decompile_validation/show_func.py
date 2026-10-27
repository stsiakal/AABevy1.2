"""Print one Ghidra function body (or a byte range of it) by address."""

import os
import re
import sys

HEADER_RE = re.compile(r"^// ==== (\S+) @ ([0-9a-fA-F]+)\s*$")


def main():
    if len(sys.argv) < 2:
        print("usage: show_func.py <addr> [start] [count]")
        return 2
    want = int(sys.argv[1], 16)
    start = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    count = int(sys.argv[3]) if len(sys.argv) > 3 else 200
    mod = sys.argv[4] if len(sys.argv) > 4 else "x2game"
    paths = {
        "x2game": "D:/aa-re/client12/out-x2game/x2game.dll.c",
        "cryaction": "D:/aa-re/client12/engine/out/cryaction.dll.c",
        "cryanimation": "D:/aa-re/client12/engine/out/cryanimation.dll.c",
        "cryentitysystem": "D:/aa-re/client12/engine/out/cryentitysystem.dll.c",
        "crygame": "D:/aa-re/client12/engine/out/crygame.dll.c",
        "crysystem": "D:/aa-re/client12/engine3/out/crysystem.dll.c",
        "x2ui": "D:/aa-re/client12/engine3/out/x2ui.dll.c",
    }
    path = paths[mod]
    body = None
    header = None
    buf = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            m = HEADER_RE.match(line.rstrip("\n"))
            if m:
                if body is not None:
                    break
                if int(m.group(2), 16) == want:
                    body = True
                    header = line.rstrip("\n")
                    continue
                continue
            if body is not None:
                buf.append(line.rstrip("\n"))
    if body is None:
        print("not found: %08x" % want)
        return 1
    print(header)
    for i, line in enumerate(buf[start:start + count], start + 1):
        print("%4d %s" % (i, line))
    return 0


if __name__ == "__main__":
    sys.exit(main())
