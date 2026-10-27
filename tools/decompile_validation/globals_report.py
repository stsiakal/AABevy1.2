"""Classify every DAT_/vtable slot cited by the ports.

Evidence classes:
  STATIC   value/section readable from the unpacked DLL (.rdata/.text)
  RUNTIME  .data/.bss pointer, zero in the dump; value only exists live
  LIVE     port comment cites a live capture/golden/probe
  ASSUMED  port text admits an inference/placeholder, or no evidence
"""

import argparse
import csv
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pe_value

LIVE_RE = re.compile(r"live|captur|golden|probe|observ|measur|dump(?:ed)?", re.I)
ASSUME_RE = re.compile(r"assum|guess|placeholder|unverif|unknown|infer|presum", re.I)


def rows_of(path):
    with open(path, encoding="utf-8") as f:
        yield from csv.DictReader(f, delimiter="\t")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    args = ap.parse_args()
    root = os.path.abspath(args.root)
    refs = os.path.join(root, "data", "decompile_validation", "refs.tsv")

    dats, vts = {}, {}
    for r in rows_of(refs):
        if r["kind"] == "dat":
            d = dats.setdefault(r["addr"], {"port": [], "map": []})
            d["port" if r["source"] == "port" else "map"].append(r)
        elif r["kind"] in ("vtable", "slot"):
            a = r["addr"].lstrip("+")
            while a.startswith("0x0x"):
                a = a[2:]
            if not a.startswith("0x"):
                a = "0x" + a
            d = vts.setdefault(a, {"port": [], "map": []})
            d["port" if r["source"] == "port" else "map"].append(r)

    out = os.path.join(root, "data", "decompile_validation", "globals.tsv")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("addr\tsection\tvalue\tclass\tport_refs\tevidence\tmap_refs\n")
        for addr in sorted(dats):
            d = dats[addr]
            if not d["port"]:
                continue
            va = int(addr, 16)
            hits = pe_value.lookup(va)
            section = ",".join("%s:%s" % (m, s) for m, s, _ in hits) or "-"
            vals = []
            for m, s, v in hits:
                if v:
                    vals.append("%s=%s(f32 %.9g)" % (m, v["hex"][:8], v["f32"]))
            value = ";".join(vals) or "-"
            texts = " ".join(r["text"] for r in d["port"])
            nonzero = any(v and v["hex"][:16].strip("0") for _, _, v in hits)
            if ASSUME_RE.search(texts):
                cls = "ASSUMED"
            elif any(s.startswith(".rdata") for _, s, _ in hits):
                cls = "STATIC"
            elif any(s.startswith(".data") for _, s, _ in hits):
                if nonzero:
                    cls = "STATIC"
                elif LIVE_RE.search(texts):
                    cls = "LIVE"
                else:
                    cls = "RUNTIME"
            elif hits:
                cls = "TEXT"
            else:
                cls = "ASSUMED"
            port_refs = ";".join("%s:%s" % (r["file"], r["line"]) for r in d["port"][:6])
            evidence = " | ".join(dict.fromkeys(r["text"] for r in d["port"][:3]))[:400]
            map_refs = ";".join("%s:%s" % (r["file"], r["line"]) for r in d["map"][:4])
            f.write("%s\t%s\t%s\t%s\t%s\t%s\t%s\n" % (
                addr, section, value, cls, port_refs, evidence, map_refs))

    vout = os.path.join(root, "data", "decompile_validation", "vtables.tsv")
    with open(vout, "w", encoding="utf-8", newline="\n") as f:
        f.write("slot\tclass\tport_refs\tevidence\tmap_refs\n")
        for addr in sorted(vts, key=lambda x: int(x, 16)):
            d = vts[addr]
            if not d["port"]:
                continue
            texts = " ".join(r["text"] for r in d["port"])
            if ASSUME_RE.search(texts):
                cls = "ASSUMED"
            elif LIVE_RE.search(texts):
                cls = "LIVE"
            else:
                cls = "STATIC"
            f.write("%s\t%s\t%s\t%s\t%s\n" % (
                addr, cls,
                ";".join("%s:%s" % (r["file"], r["line"]) for r in d["port"][:6]),
                " | ".join(dict.fromkeys(r["text"] for r in d["port"][:3]))[:400],
                ";".join("%s:%s" % (r["file"], r["line"]) for r in d["map"][:4])))

    print("DAT_ with port refs=%d, vtable slots with port refs=%d" % (
        sum(1 for d in dats.values() if d["port"]),
        sum(1 for d in vts.values() if d["port"])))


if __name__ == "__main__":
    sys.exit(main())
