"""Collect every function/global address cited by the ports and maps.

Outputs TSV rows: kind, addr, symbol, module_hint, source, line, text

`kind` is func, dat, vtable, or const. Only 8-digit hex values are treated as
possible VAs; 32-bit constants such as fourcc/event hashes are kept as
`const` so they can be separated by the function index.
"""

import argparse
import os
import re
import sys

FUNC_RE = re.compile(r"\bFUN_([0-9a-fA-F]{8})\b")
DAT_RE = re.compile(r"\bDAT_([0-9a-fA-F]{8})\b")
ADDR_RE = re.compile(r"\b0x([0-9a-fA-F]{8})\b")
VT_RE = re.compile(r"(?:vt|vtable)[^0-9a-fA-F]{0,8}(0x[0-9a-fA-F]+)", re.I)
SLOT_RE = re.compile(r"\bvt\[?(0x[0-9a-fA-F]+|\+0x[0-9a-fA-F]+)\]?")
SIZE_RE = re.compile(r"\bVA\s+`?0x([0-9a-fA-F]{8})`?[^\n]{0,40}?(\d+)\s*B\b")


def module_hint(path):
    p = path.replace("\\", "/").lower()
    for key, mod in (
        ("port-engine-anim", "cryanimation/cryaction"),
        ("port-engine-char", "cryentitysystem"),
        ("port-engine-combat", "cryaction/crygame"),
        ("port-engine-loop", "cryaction/crygame"),
        ("port-mount-anim", "cryanimation/x2game"),
        ("port-movement", "x2game"),
        ("port-combat", "x2game"),
        ("port-interaction", "x2game"),
        ("port-movement-extra", "x2game"),
        ("port-vehicles", "x2game"),
        ("port-camera", "x2game"),
        ("port-formulas", "x2game"),
    ):
        if key in p:
            return mod
    return ""


def walk(root, suffix):
    for base, _, files in os.walk(root):
        for fn in files:
            if fn.endswith(suffix):
                yield os.path.join(base, fn)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--out", default="data/decompile_validation/refs.tsv")
    args = ap.parse_args()

    root = os.path.abspath(args.root)
    ports = os.path.join(root, "crates", "aa-client", "src", "port")
    maps = os.path.join(root, "docs", "research")
    rows = []

    def scan(path, kind_src):
        rel = os.path.relpath(path, root)
        with open(path, encoding="utf-8", errors="replace") as f:
            for lineno, line in enumerate(f, 1):
                for m in FUNC_RE.finditer(line):
                    rows.append(("func", m.group(1).lower(), "FUN_" + m.group(1).lower(),
                                 module_hint(rel), kind_src, rel, str(lineno), line.strip()))
                for m in DAT_RE.finditer(line):
                    rows.append(("dat", m.group(1).lower(), "DAT_" + m.group(1).lower(),
                                 module_hint(rel), kind_src, rel, str(lineno), line.strip()))
                for m in ADDR_RE.finditer(line):
                    addr = m.group(1).lower()
                    # FUN_/DAT_ occurrences are reported above; avoid duplicates
                    if ("FUN_" + addr) in line or ("DAT_" + addr) in line:
                        continue
                    rows.append(("const", addr, "", module_hint(rel), kind_src,
                                 rel, str(lineno), line.strip()))
                for m in VT_RE.finditer(line):
                    rows.append(("vtable", m.group(1).lower(), "", module_hint(rel),
                                 kind_src, rel, str(lineno), line.strip()))
                for m in SLOT_RE.finditer(line):
                    rows.append(("slot", m.group(1).lower(), "", module_hint(rel),
                                 kind_src, rel, str(lineno), line.strip()))

    for path in walk(ports, ".rs"):
        if os.sep + "goldens" + os.sep in path:
            continue
        scan(path, "port")
    for path in walk(maps, "MAP.md"):
        scan(path, "map")

    # size claims on the same line as a FUN address
    sizes = []
    for path in walk(ports, ".rs"):
        if os.sep + "goldens" + os.sep in path:
            continue
        rel = os.path.relpath(path, root)
        for lineno, line in enumerate(open(path, encoding="utf-8", errors="replace"), 1):
            for m in SIZE_RE.finditer(line):
                sizes.append((m.group(1).lower(), m.group(2), rel, str(lineno)))
    for path in walk(maps, "MAP.md"):
        rel = os.path.relpath(path, root)
        for lineno, line in enumerate(open(path, encoding="utf-8", errors="replace"), 1):
            for m in SIZE_RE.finditer(line):
                sizes.append((m.group(1).lower(), m.group(2), rel, str(lineno)))

    out = os.path.join(root, args.out)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("kind\taddr\tsymbol\tmodule_hint\tsource\tfile\tline\ttext\n")
        for r in rows:
            f.write("\t".join(r) + "\n")
    with open(os.path.join(os.path.dirname(out), "sizes.tsv"), "w", encoding="utf-8", newline="\n") as f:
        f.write("addr\tclaimed_size\tfile\tline\n")
        for r in sizes:
            f.write("\t".join(r) + "\n")

    uniq = sorted({(r[0], r[1]) for r in rows})
    print("rows=%d unique=%d sizes=%d -> %s" % (len(rows), len(uniq), len(sizes), out))


if __name__ == "__main__":
    sys.exit(main())
