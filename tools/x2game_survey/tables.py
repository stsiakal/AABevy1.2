#!/usr/bin/env python3
"""Dump SQL table names and Load* names per DB-region address interval."""
import collections
import csv
import re

CSV = r"D:/aa-re/client12/out-x2game/funcs.csv"
XREFS = r"D:/aa-re/client12/out-x2game/strings-xrefs.tsv"

B = [
    (0x39571FB0, 0x3957D870),
    (0x3957D870, 0x395AC210),
    (0x395AC210, 0x395B9000),
    (0x395B9000, 0x395D0000),
    (0x395D0000, 0x395F0000),
    (0x395F0000, 0x39610000),
    (0x39610000, 0x39630000),
    (0x39630000, 0x39660000),
    (0x39660000, 0x39680000),
    (0x39680000, 0x396A0000),
    (0x396A0000, 0x396C0000),
    (0x396C0000, 0x396D0000),
    (0x396D0000, 0x396E0000),
    (0x396E0000, 0x397135D0),
]
TAB = re.compile(r"(?:FROM|INTO|UPDATE|TABLE|JOIN)\s+([A-Za-z_][A-Za-z0-9_]*)", re.I)
LOAD = re.compile(r"function=(Load[A-Za-z0-9_]+)")


def main():
    sizes = {}
    for r in csv.DictReader(open(CSV)):
        s = int(r["start"], 16)
        if 0x394BEC40 <= s < 0x397135D0:
            sizes[s] = int(r["size"])
    xr = collections.defaultdict(list)
    for r in csv.DictReader(
        open(XREFS, encoding="utf-8", errors="replace"), delimiter="\t"
    ):
        fa = r["function_addr"]
        if fa != "-":
            a = int(fa, 16)
            if a in sizes:
                xr[a].append(r["string"])
    for lo, hi in B:
        tabs = collections.Counter()
        loads = set()
        for a in sizes:
            if lo <= a < hi:
                for s in xr.get(a, []):
                    for m in TAB.finditer(s):
                        t = m.group(1)
                        if t.lower() not in ("select", "where", "and", "or"):
                            tabs[t] += 1
                    m = LOAD.search(s)
                    if m:
                        loads.add(m.group(1))
        print("== %08x tables: %s" % (lo, ",".join(t for t, _ in tabs.most_common(30))))
        if loads:
            print("   loads: %s" % ",".join(sorted(loads))[:800])


if __name__ == "__main__":
    main()
