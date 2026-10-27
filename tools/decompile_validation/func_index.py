"""Index Ghidra/IDA function lists and compare them with port size claims.

Inputs (read-only):
  D:/aa-re/client12/out-x2game/funcs.csv
  D:/aa-re/client12/engine*/out/*.funcs.csv
  D:/aa-re/client12/ida/*.funcs.csv
  D:/aa-re/scratch/ida12-x2game/out/*.funcs.csv   (created by ida_func_export.py)
  data/decompile_validation/refs.tsv, sizes.tsv

Output:
  data/decompile_validation/functions.tsv
"""

import argparse
import csv
import glob
import os
import re
import sys

GHIDRA_DIRS = [
    "D:/aa-re/client12/out-x2game",
    "D:/aa-re/client12/engine/out",
    "D:/aa-re/client12/engine2/out",
    "D:/aa-re/client12/engine3/out",
]
IDA_DIRS = [
    "D:/aa-re/client12/ida",
    "D:/aa-re/scratch/ida12-x2game/out",
]

DECL_RE = re.compile(
    r"^// ==== (FUN_[0-9a-fA-F]+|thunk_[^ ]+|[A-Za-z_][\w@$]*)\s*@\s*([0-9a-fA-F]+)\s*$"
)


def parse_int(s):
    s = s.strip()
    return int(s, 16)


def module_of(path):
    name = os.path.basename(path)
    if name == "funcs.csv":
        return "x2game"
    return name.rsplit(".", 2)[0]


def load_funcs(dirs):
    out = []
    for d in dirs:
        paths = sorted(glob.glob(os.path.join(d, "*.funcs.csv")))
        paths += sorted(glob.glob(os.path.join(d, "funcs.csv")))
        for path in paths:
            mod = module_of(path)
            with open(path, encoding="utf-8", errors="replace", newline="") as f:
                r = csv.reader(f)
                header = next(r)
                idx = {h.strip(): i for i, h in enumerate(header)}
                for row in r:
                    if len(row) < 4:
                        continue
                    try:
                        start = parse_int(row[idx["start"]])
                        end = parse_int(row[idx["end"]])
                        size = int(row[idx["size"]])
                    except (ValueError, KeyError):
                        continue
                    entry = {
                        "module": mod,
                        "start": start,
                        "end": end,
                        "size": size,
                        "name": row[idx["name"]],
                        "source": path,
                    }
                    if "args_bytes" in idx and len(row) > idx["args_bytes"]:
                        try:
                            entry["args_bytes"] = int(row[idx["args_bytes"]])
                        except ValueError:
                            entry["args_bytes"] = None
                    else:
                        entry["args_bytes"] = None
                    if "first_ops" in idx and len(row) > idx["first_ops"]:
                        entry["first_ops"] = row[idx["first_ops"]]
                    else:
                        entry["first_ops"] = ""
                    out.append(entry)
    return out


def load_ghidra_decls(path):
    """Return {addr: (name, params)} for the x2game decompile."""
    decls = {}
    cur = None
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            m = DECL_RE.match(line.rstrip("\n"))
            if m:
                cur = int(m.group(2), 16)
                continue
            if cur is None:
                continue
            s = line.strip()
            if not s or s.startswith("/*") or s.startswith("//") or s.startswith("*"):
                continue
            if "(" in s and ")" in s:
                head = s.split("(", 1)[0]
                if "FUN_" in head or "__thiscall" in head or "__stdcall" in head or "__cdecl" in head:
                    args = s.split("(", 1)[1].rsplit(")", 1)[0].strip()
                    params = 0 if args in ("", "void") else len(split_args(args))
                    decls[cur] = (head.strip().split()[-1], params)
                    cur = None
                    continue
            # undefined prototypes (e.g. `void FUN_x(void)`) still match above
            if "{" in s:
                cur = None
    return decls


def split_args(s):
    parts, depth, cur = [], 0, ""
    for ch in s:
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
            continue
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        cur += ch
    if cur.strip():
        parts.append(cur)
    return parts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--out", default="data/decompile_validation/functions.tsv")
    args = ap.parse_args()
    root = os.path.abspath(args.root)

    ghidra = load_funcs(GHIDRA_DIRS)
    ida = load_funcs(IDA_DIRS)
    ghidra_by_start = {}
    for e in ghidra:
        ghidra_by_start.setdefault(e["start"], []).append(e)
    ida_by_start = {}
    for e in ida:
        ida_by_start.setdefault(e["start"], []).append(e)

    decls = load_ghidra_decls("D:/aa-re/client12/out-x2game/x2game.dll.c")

    claims = {}
    with open(os.path.join(root, "data/decompile_validation/sizes.tsv"), encoding="utf-8") as f:
        r = csv.DictReader(f, delimiter="\t")
        for row in r:
            claims.setdefault(int(row["addr"], 16), []).append(row)

    refs = {}
    with open(os.path.join(root, "data/decompile_validation/refs.tsv"), encoding="utf-8") as f:
        r = csv.DictReader(f, delimiter="\t")
        for row in r:
            if row["kind"] not in ("func", "const"):
                continue
            try:
                addr = int(row["addr"], 16)
            except ValueError:
                continue
            if addr not in ghidra_by_start and addr not in ida_by_start:
                continue
            refs.setdefault(addr, []).append(row)

    out = os.path.join(root, args.out)
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("addr\tghidra_module\tghidra_size\tghidra_params\tida_module\tida_size\tida_args_bytes\tida_first_ops\tclaimed_sizes\tstatus\tport_refs\n")
        for addr in sorted(refs):
            g = ghidra_by_start.get(addr, [])
            i = ida_by_start.get(addr, [])
            gmods = ",".join(sorted({e["module"] for e in g})) or "-"
            imods = ",".join(sorted({e["module"] for e in i})) or "-"
            gsizes = ",".join(str(e["size"]) for e in g) or "-"
            params = ""
            if g:
                name, params_n = decls.get(addr, ("", None))
                params = "" if params_n is None else str(params_n)
            isizes = ",".join(str(e["size"]) for e in i) or "-"
            iargs = ",".join(
                "" if e.get("args_bytes") is None else str(e["args_bytes"]) for e in i
            ) or "-"
            iops = " | ".join(e.get("first_ops", "") for e in i)
            c = claims.get(addr, [])
            csizes = ",".join(sorted({row["claimed_size"] for row in c})) or "-"
            status = []
            if len(g) > 1:
                status.append("AMBIGUOUS_GHIDRA")
            if len(i) > 1:
                status.append("AMBIGUOUS_IDA")
            if g and i:
                if {e["size"] for e in g} != {e["size"] for e in i}:
                    status.append("SIZE_MISMATCH_IDA")
            elif g and not i:
                status.append("NOT_IN_IDA")
            if c:
                claimed = {int(row["claimed_size"]) for row in c}
                actual = {e["size"] for e in g}
                if actual and claimed != actual:
                    status.append("SIZE_MISMATCH_CLAIM")
            port_refs = ";".join(
                "%s:%s" % (row["file"], row["line"]) for row in refs[addr][:8]
            )
            f.write(
                "%08x\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n"
                % (addr, gmods, gsizes, params, imods, isizes, iargs, iops,
                   csizes, ",".join(status), port_refs)
            )

    with open(os.path.join(root, "data/decompile_validation/refs_unmatched.tsv"), "w", encoding="utf-8", newline="\n") as f:
        f.write("kind\taddr\tfile\tline\n")
        n = 0
        for addr, rows in sorted(refs.items()):
            pass
        for kind in ("func", "const"):
            for row in refs.get(0, []):
                pass
        # recompute unmatched by scanning the inputs again
        with open(os.path.join(root, "data/decompile_validation/refs.tsv"), encoding="utf-8") as src:
            r = csv.DictReader(src, delimiter="\t")
            for row in r:
                if row["kind"] not in ("func", "const"):
                    continue
                addr = int(row["addr"], 16)
                if addr not in ghidra_by_start and addr not in ida_by_start:
                    f.write("%s\t%s\t%s\t%s\n" % (row["kind"], row["addr"], row["file"], row["line"]))
                    n += 1
    print("functions=%d -> %s (unmatched refs=%d)" % (len(refs), out, n))


if __name__ == "__main__":
    sys.exit(main())
