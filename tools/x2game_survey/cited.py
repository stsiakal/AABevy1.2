#!/usr/bin/env python3
"""Collect hex code addresses cited by existing maps/ports (read-only)."""
import os
import re

ROOTS = [
    r"D:/aaemu-client/wt/glide/docs/research",
    r"D:/aaemu-client/wt/glide/crates/aa-client/src/port",
]
PAT = re.compile(r"(?i)(?<![0-9a-f])(?:0x)?(39[0-9a-f]{6})(?![0-9a-f])")


def main():
    hits = {}
    for root in ROOTS:
        for dirpath, _dirs, files in os.walk(root):
            for fn in files:
                if not fn.endswith((".md", ".rs", ".txt")):
                    continue
                p = os.path.join(dirpath, fn)
                try:
                    txt = open(p, encoding="utf-8", errors="replace").read()
                except OSError:
                    continue
                for m in PAT.finditer(txt):
                    a = int(m.group(1), 16)
                    hits.setdefault(a, set()).add(os.path.relpath(p, r"D:/aaemu-client/wt/glide"))
    out = r"D:/aaemu-client/wt/glide/data/extract/x2game/cited_addrs.tsv"
    with open(out, "w", encoding="utf-8") as f:
        for a in sorted(hits):
            f.write("%08x\t%s\n" % (a, ",".join(sorted(hits[a]))))
    print("addresses", len(hits), "->", out)


if __name__ == "__main__":
    main()
