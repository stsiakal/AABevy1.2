"""List Ghidra-vs-IDA disagreements for the addresses cited by the ports.

Kinds:
  SIZE       function boundary differs (IDA size != Ghidra size)
  PARAMS     Ghidra parameter count cannot explain IDA's ret-imm16 bytes
             under the declared calling convention (or the decompile reads
             unnamed in_stack_* arguments)
  NO_IDA     Ghidra has a function, IDA has none at that address
  NO_BODY    x2game function has no decompile body in x2game.dll.c
"""

import argparse
import csv
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    args = ap.parse_args()
    root = os.path.abspath(args.root)
    base = os.path.join(root, "data", "decompile_validation")

    bodies = {}
    with open(os.path.join(base, "x2game_funcs.tsv"), encoding="utf-8") as f:
        for r in csv.DictReader(f, delimiter="\t"):
            bodies[r["addr"]] = r

    rows = list(csv.DictReader(open(os.path.join(base, "functions.tsv"), encoding="utf-8"), delimiter="\t"))
    out = []
    for r in rows:
        refs = r["port_refs"]
        addr = r["addr"]
        g, i = r["ghidra_size"], r["ida_size"]
        if r["ghidra_module"] == "x2game" and addr not in bodies:
            out.append((addr, "NO_BODY", g, i, "", refs))
        if r["ghidra_module"] != "-" and r["ida_module"] == "-":
            out.append((addr, "NO_IDA", g, "-", "", refs))
            continue
        if r["ghidra_module"] == "-" or r["ida_module"] == "-":
            continue
        try:
            gs, isz = int(g), int(i)
        except ValueError:
            out.append((addr, "SIZE", g, i, "ambiguous", refs))
            continue
        if gs != isz:
            out.append((addr, "SIZE", str(gs), str(isz), "delta=%+d" % (isz - gs), refs))
        b = bodies.get(addr)
        if b and b["params"] and r["ida_args_bytes"] not in ("", "-"):
            p = int(b["params"])
            a = int(r["ida_args_bytes"])
            decl = b["decl"]
            if "__thiscall" in decl:
                expected = {a // 4 + 1}
            elif "__cdecl" in decl:
                expected = {a // 4}
            elif "__fastcall" in decl:
                expected = {a // 4 + 2}
            else:
                expected = {a // 4, a // 4 + 1, a // 4 + 2}
            if p not in expected:
                out.append((addr, "PARAMS", str(p), str(a),
                            "%s expected=%s" % (decl.split("FUN_")[0].strip(),
                                                ",".join(map(str, sorted(expected)))),
                            refs))

    path = os.path.join(base, "disagreements.tsv")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("addr\tkind\tghidra\tida\tdetail\tport_refs\n")
        for r in out:
            f.write("\t".join(r) + "\n")
    from collections import Counter
    print("disagreements=%d %s -> %s" % (len(out), dict(Counter(r[1] for r in out)), path))


if __name__ == "__main__":
    sys.exit(main())
