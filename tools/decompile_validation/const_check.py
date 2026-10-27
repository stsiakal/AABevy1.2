"""Cross-check numeric constants in the ports against the unpacked .data/.rdata.

Pairs the DAT_ names in a doc-comment block with the `pub const` lines that
follow it, in order (only when the counts match), then compares the port value
with the bytes in the DLL.
"""

import argparse
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pe_value

DAT_RE = re.compile(r"DAT_([0-9a-fA-F]{8})")
CONST_RE = re.compile(
    r"pub const (\w+)\s*:\s*(i32|u32|f32|f64|usize)\s*=\s*([^;]+);")
INT_RE = re.compile(r"^(0x[0-9a-fA-F_]+|[0-9_]+)$")
FLOAT_RE = re.compile(r"^(-?[0-9][0-9_]*(?:\.[0-9_]*)?(?:e-?[0-9]+)?)f32$")


def close(a, b, rel=1e-5):
    if b == 0:
        return abs(a) < 1e-9
    return abs(a - b) <= rel * abs(b)


def blocks(lines):
    i = 0
    while i < len(lines):
        if not lines[i].lstrip().startswith("///"):
            i += 1
            continue
        start = i
        while i < len(lines) and lines[i].lstrip().startswith("///"):
            i += 1
        doc = "\n".join(lines[start:i])
        consts = []
        j = i
        while j < len(lines):
            s = lines[j].strip()
            if not s:
                j += 1
                continue
            m = CONST_RE.search(lines[j])
            if not m:
                break
            consts.append((j + 1, m.group(1), m.group(2), m.group(3).strip()))
            j += 1
        dats = DAT_RE.findall(doc)
        if consts and dats and len(consts) == len(dats):
            for (lineno, name, ty, expr), dat in zip(consts, dats):
                yield lineno, name, ty, expr, dat
        i = max(i, j)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    args = ap.parse_args()
    root = os.path.abspath(args.root)
    port = os.path.join(root, "crates", "aa-client", "src", "port")
    rows = []
    for base, _, files in os.walk(port):
        for fn in files:
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(base, fn)
            rel = os.path.relpath(path, root)
            lines = open(path, encoding="utf-8", errors="replace").read().splitlines()
            for lineno, name, ty, expr, dat in blocks(lines):
                va = int(dat, 16)
                hits = pe_value.lookup(va)
                if not hits:
                    rows.append((rel, str(lineno), name, dat, expr, "NO_BYTES", "assumed"))
                    continue
                mod, sec, val = hits[0]
                if not val:
                    rows.append((rel, str(lineno), name, dat, expr, "NO_BYTES", "assumed"))
                    continue
                status = "OK"
                if ty in ("i32", "u32", "usize"):
                    im = INT_RE.match(expr)
                    if im:
                        want = int(im.group(1).replace("_", ""), 0)
                        got = val["i32"] if ty == "i32" else val["u32"]
                        if want != got:
                            status = "MISMATCH want=%s got=%s" % (want, got)
                    else:
                        status = "EXPR"
                elif ty == "f32":
                    fm = FLOAT_RE.match(expr)
                    if fm:
                        want = float(fm.group(1).replace("_", ""))
                        if not close(want, val["f32"]):
                            status = "MISMATCH want=%s got=%s" % (want, val["f32"])
                    else:
                        status = "EXPR"
                rows.append((rel, str(lineno), name, dat, expr, status,
                             "%s:%s=%s" % (mod, sec, val["hex"][:8])))

    out = os.path.join(root, "data", "decompile_validation", "const_check.tsv")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("file\tline\tname\tDAT\texpr\tstatus\tbytes\n")
        for r in rows:
            f.write("\t".join(r) + "\n")
    bad = [r for r in rows if r[5].startswith("MISMATCH")]
    print("constants=%d mismatches=%d -> %s" % (len(rows), len(bad), out))
    for r in bad:
        print("%s:%s %s %s %s -> %s (%s)" % (r[0], r[1], r[2], r[3], r[4], r[5], r[6]))


if __name__ == "__main__":
    sys.exit(main())
