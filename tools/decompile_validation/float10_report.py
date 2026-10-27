"""Flag cited x2game functions whose decompile contains x87 float10 math.

Also reports each port module's test style (exact assert_eq vs tolerance) so an
exact test over a float10 path is visible.
"""

import argparse
import csv
import os
import re
import sys

FLOAT_TOL = re.compile(r"(?:abs\(\)\s*<|TOL|EPS|epsilon|approx|is_close|relative_eq|abs_diff)")
EXACT = re.compile(r"assert_eq!|assert_ne!")
APPROX = re.compile(r"assert!\([^\n]*(?:abs\(\)|abs_diff|delta|tol|eps)", re.I)


def read_tsv(path):
    with open(path, encoding="utf-8") as f:
        yield from csv.DictReader(f, delimiter="\t")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--out", default="data/decompile_validation/float10.tsv")
    args = ap.parse_args()
    root = os.path.abspath(args.root)
    base = os.path.join(root, "data", "decompile_validation")

    bodies = {}
    for r in read_tsv(os.path.join(base, "x2game_funcs.tsv")):
        bodies[r["addr"]] = r

    rows = []
    for r in read_tsv(os.path.join(base, "functions.tsv")):
        if r["ghidra_module"] != "x2game":
            continue
        b = bodies.get(r["addr"])
        if not b:
            rows.append([r["addr"], r["port_refs"], "NO_DECOMPILE", "", "", ""])
            continue
        if int(b["float10"]) > 0:
            rows.append([r["addr"], r["port_refs"], b["float10"], b["params"],
                         b["start_line"], r["ghidra_size"]])

    out = os.path.join(root, args.out)
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("addr\tport_refs\tfloat10\tparams\tstart_line\tghidra_size\n")
        for r in sorted(rows, key=lambda x: (-int(x[2]) if str(x[2]).isdigit() else -999)):
            f.write("\t".join(str(x) for x in r) + "\n")

    # module test style
    port = os.path.join(root, "crates", "aa-client", "src", "port")
    styles = []
    for mod in sorted(os.listdir(port)):
        d = os.path.join(port, mod)
        if not os.path.isdir(d):
            continue
        t = os.path.join(d, "tests.rs")
        if not os.path.exists(t):
            continue
        text = open(t, encoding="utf-8", errors="replace").read()
        styles.append((mod, len(EXACT.findall(text)), len(APPROX.findall(text)),
                       len(FLOAT_TOL.findall(text))))
    with open(os.path.join(os.path.dirname(out), "test_styles.tsv"), "w", encoding="utf-8", newline="\n") as f:
        f.write("module\tassert_eq\tapprox\tfloat_tol\n")
        for s in styles:
            f.write("%s\t%d\t%d\t%d\n" % s)

    print("float10 cited x2game functions=%d -> %s" % (len(rows), out))


if __name__ == "__main__":
    sys.exit(main())
