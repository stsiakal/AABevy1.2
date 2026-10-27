#!/usr/bin/env python3
"""Range survey helper for the x2game.dll classification.

Reads the read-only decompile export under D:/aa-re/client12/out-x2game and
prints/creates per-range summaries (functions, calls, strings).  Never reads
the whole .c file: functions are sliced by their "// ==== FUN_<addr>" header.

usage:
  python -I survey.py range <lo> <hi> <outdir>
  python -I survey.py blocks <lo> <hi>
  python -I survey.py fn <addr>          # slice one function from the .c
  python -I survey.py strs <lo> <hi> [substr]   # string xrefs in range
"""
import csv
import os
import sys

OUT = r"D:/aa-re/client12/out-x2game"
FUNCS = os.path.join(OUT, "funcs.csv")
CALLS = os.path.join(OUT, "calls.tsv")
XREFS = os.path.join(OUT, "strings-xrefs.tsv")
DEC = os.path.join(OUT, "x2game.dll.c")


def load_funcs(lo, hi):
    rows = []
    with open(FUNCS, newline="", encoding="utf-8", errors="replace") as f:
        for r in csv.DictReader(f):
            s = int(r["start"], 16)
            if lo <= s < hi:
                rows.append(
                    {
                        "start": s,
                        "end": int(r["end"], 16),
                        "size": int(r["size"]),
                        "name": r["name"],
                    }
                )
    rows.sort(key=lambda r: r["start"])
    return rows


def load_xrefs(lo, hi):
    by_fn = {}
    with open(XREFS, newline="", encoding="utf-8", errors="replace") as f:
        for r in csv.DictReader(f, delimiter="\t"):
            fa = r["function_addr"]
            if fa == "-":
                continue
            a = int(fa, 16)
            if lo <= a < hi:
                by_fn.setdefault(a, []).append(r["string"])
    return by_fn


def load_calls(lo, hi):
    out_edges = []
    in_edges = {}
    with open(CALLS, newline="", encoding="utf-8", errors="replace") as f:
        for r in csv.DictReader(f, delimiter="\t"):
            a = int(r["caller_addr"], 16)
            if lo <= a < hi:
                ca = r["callee_addr"]
                try:
                    b = int(ca, 16)
                except ValueError:
                    b = -1
                out_edges.append((a, b, r["callee_name"]))
                in_edges.setdefault(b, []).append(a)
    return out_edges, in_edges


def cmd_range(lo, hi, outdir):
    fns = load_funcs(lo, hi)
    xr = load_xrefs(lo, hi)
    oe, ie = load_calls(lo, hi)
    os.makedirs(outdir, exist_ok=True)
    total = sum(f["size"] for f in fns)
    with open(os.path.join(outdir, "funcs.tsv"), "w", encoding="utf-8") as f:
        f.write("start\tend\tsize\tn_in_range\tn_out_range\tstrings\n")
        internal = {f["start"] for f in fns}
        for fn in fns:
            o = sum(1 for a, b, _ in oe if a == fn["start"] and b in internal)
            s = " | ".join(dict.fromkeys(xr.get(fn["start"], [])))
            f.write(
                "%08x\t%08x\t%d\t%d\t%d\t%s\n"
                % (fn["start"], fn["end"], fn["size"], len(ie.get(fn["start"], [])), o, s)
            )
    print("functions %d bytes %d (%.1f KB)" % (len(fns), total, total / 1024))
    return 0


def cmd_blocks(lo, hi):
    fns = load_funcs(lo, hi)
    xr = load_xrefs(lo, hi)
    cur = None
    for fn in fns:
        blk = fn["start"] >> 16
        if blk != cur:
            cur = blk
            n = sum(1 for f in fns if f["start"] >> 16 == blk)
            b = sum(f["size"] for f in fns if f["start"] >> 16 == blk)
            print("\n== %08x  fns=%d KB=%.1f" % (blk << 16, n, b / 1024))
        ss = list(dict.fromkeys(xr.get(fn["start"], [])))
        if ss:
            print("%08x %5d %s" % (fn["start"], fn["size"], " | ".join(ss)[:220]))
    return 0


def cmd_strs(lo, hi, needle=None):
    fns = {f["start"] for f in load_funcs(lo, hi)}
    with open(XREFS, newline="", encoding="utf-8", errors="replace") as f:
        for r in csv.DictReader(f, delimiter="\t"):
            fa = r["function_addr"]
            if fa == "-":
                continue
            a = int(fa, 16)
            if a in fns and (needle is None or needle.lower() in r["string"].lower()):
                print("%08x %s %r" % (a, r["string_addr"], r["string"]))
    return 0


def cmd_fn(addr):
    addrs = sorted(addr)
    want = addrs[0]
    started = False
    with open(DEC, encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith("// ==== FUN_"):
                a = int(line.split("@")[1].split()[0], 16)
                if started:
                    break
                if a == want:
                    started = True
            if started:
                sys.stdout.write(line)
    return 0


def main():
    cmd = sys.argv[1]
    if cmd == "range":
        return cmd_range(int(sys.argv[2], 16), int(sys.argv[3], 16), sys.argv[4])
    if cmd == "blocks":
        return cmd_blocks(int(sys.argv[2], 16), int(sys.argv[3], 16))
    if cmd == "strs":
        return cmd_strs(
            int(sys.argv[2], 16),
            int(sys.argv[3], 16),
            sys.argv[4] if len(sys.argv) > 4 else None,
        )
    if cmd == "fn":
        return cmd_fn([int(x, 16) for x in sys.argv[2:]])
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
