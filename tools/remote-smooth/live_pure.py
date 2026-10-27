"""Live-proof the pure remote-smoothing leaves against the hidden original 1.2 client.

Calls FUN_39097dc0 (f32 SmoothCD) and FUN_390a0ad0 (f64 SmoothCD) with crafted buffers
through the aaemu-clientdriver call harness and writes the golden JSON used by
crates/aa-client/src/port/remote/tests.rs. One MCP session owns the hidden client for the
whole run (closing the session stops it).
"""
import json
import os
import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(r"D:/aaemu-client/wt/pr")
GOLD = ROOT / "crates/aa-client/src/port/remote/goldens"
T = r"D:/aaemu-target/move-golden/debug/aaemu-clientdriver.exe"
DLL = r"D:/aaemu-target/move-golden/aadriver/aadriver.dll"
CLIENT = r"D:/AA1.2/bin32/archeage.exe"
LAUNCHER = r"D:/aaemu-live-move-golden/bin/aaemu-launcher.exe"
PW = r"D:/aaemu-live-move-golden/cap-pw.txt"
ART = Path(r"C:/Users/stsiakal/AppData/Local/Temp/opencode/pure-mcp")

sys.path.insert(0, str(ROOT / "tools/feel-recorder"))
from run import Driver  # noqa: E402


def f32hex(vals):
    return b"".join(struct.pack("<f", v) for v in vals).hex()


def f64hex(vals):
    return b"".join(struct.pack("<d", v) for v in vals).hex()


def hexfloats32(s):
    return list(struct.unpack("<%df" % (len(s) // 8), bytes.fromhex(s)))


def hexfloats64(s):
    return list(struct.unpack("<%dd" % (len(s) // 16), bytes.fromhex(s)))


class Cli:
    def __init__(self, pid, wait=240):
        self.pid = pid
        deadline = time.monotonic() + wait
        while True:
            try:
                self.mod()
                return
            except RuntimeError:
                if time.monotonic() > deadline:
                    raise
                time.sleep(2)

    def run(self, *args):
        p = subprocess.run([T, *map(str, args), "--pid", str(self.pid)],
                           capture_output=True, text=True, timeout=120)
        if p.returncode != 0:
            raise RuntimeError(f"{args[0]}: {p.stderr.strip() or p.stdout.strip()}")
        return p.stdout.strip()

    def mod(self):
        out = self.run("mod", "x2game.dll")
        return int(out.split("base=")[1].split()[0], 16)

    def alloc(self, n):
        return int(self.run("alloc", n), 16)

    def write(self, addr, hexs):
        self.run("write", hex(addr), hexs)

    def free(self, addr):
        self.run("free", hex(addr))

    def call(self, target, args, dumps):
        argv = ["call", target, "cdecl"]
        argv += [f"dump={hex(a)}:{n}" for a, n in dumps]
        argv += [f"p:{hex(a)}" if isinstance(a, int) else (f"f:{a}" if isinstance(a, float) else str(a))
                 for a in args]
        out = self.run(*argv)
        bufs = {}
        for line in out.splitlines():
            if line.startswith("buf"):
                key, _, val = line.partition("=")
                bufs[int(key[3:])] = val.strip()
        return bufs


def smooth_f32_cases():
    A = [(1.0, 2.0, 3.0), (0.5, -0.5, 1.0)]
    T =(0.0, 0.0, 0.0)
    cases = []
    responses = [1.0, 0.5, 0.01, 10.0, 0.2, 3.5]
    dts = [0.016, 0.033, 0.1, 0.0, -0.05, 1.0]
    i = 0
    for pos in [(1.0, 2.0, 3.0), (-1.5, 0.25, 0.0), (100.0, 100.0, 5.0), (0.0, 0.0, 0.0)]:
        for vel in [(0.5, -0.5, 1.0), (0.0, 0.0, 0.0), (-2.0, 3.0, -4.0)]:
            for resp in responses:
                dt = dts[i % len(dts)]
                tgt = [(0.0, 0.0, 0.0), (2.0, -1.0, 0.5), (99.0, 101.0, 4.0)][i % 3]
                cases.append({"pos": pos, "vel": vel, "dt": dt, "target": tgt, "response": resp})
                i += 1
                if len(cases) >= 20:
                    return cases
    return cases


def smooth_f64_cases():
    cases = []
    i = 0
    for pos in [(1.0, 2.0, 3.0), (-1.5, 0.25, 0.0), (6144.0, 11265.0, 232.0), (0.0, 0.0, 0.0)]:
        for vel in [(0.5, -0.5, 1.0), (0.0, 0.0, 0.0), (-2.0, 3.0, -4.0)]:
            for resp in [1.0, 0.5, 0.01, 10.0, 0.2, 3.5]:
                dt = [0.016, 0.033, 0.1, 0.0, -0.05, 1.0][i % 6]
                tgt = [(0.0, 0.0, 0.0), (2.0, -1.0, 0.5), (6144.0, 11265.0, 232.0)][i % 3]
                cases.append({"pos": pos, "vel": vel, "dt": dt, "target": tgt, "response": resp})
                i += 1
                if len(cases) >= 20:
                    return cases
    return cases


def main():
    ART.mkdir(parents=True, exist_ok=True)
    cmd = [T, "mcp", "--dll", DLL, "--client", CLIENT, "--launcher", LAUNCHER,
           "--password-file", PW, "--login", "127.0.0.1:28237", "--user", "movegold3",
           "--artifact-dir", str(ART)]
    d = Driver(cmd)
    try:
        out = d.text("client_launch", fast_start=True, fast_pak=True, hidden=True,
                     user="movegold3", password_file=PW)
        print(out)
        import re
        pid = int(re.search(r"pid (\d+)", out).group(1))
        cli = Cli(pid)
        base = cli.mod()
        print("pid", pid, "x2game base", hex(base))
        GOLD.mkdir(parents=True, exist_ok=True)

        # f32 SmoothCD: 20 cases; dump pos and vel after the call.
        a = cli.alloc(12); b = cli.alloc(12); c = cli.alloc(12)
        out_cases = []
        for case in smooth_f32_cases():
            cli.write(a, f32hex(case["pos"]))
            cli.write(b, f32hex(case["vel"]))
            cli.write(c, f32hex(case["target"]))
            bufs = cli.call("x2game.dll+a7dc0",
                            [a, b, case["dt"], c, case["response"]],
                            [(a, 12), (b, 12)])
            out_cases.append({"in": case, "pos": f32hex(hexfloats32(bufs[0])),
                              "vel": f32hex(hexfloats32(bufs[1]))})
        (GOLD / "smooth_f32.json").write_text(json.dumps(out_cases, indent=1))
        print("f32 cases", len(out_cases))

        # f64 SmoothCD: 20 cases.
        a = cli.alloc(24); b = cli.alloc(24); c = cli.alloc(24)
        out_cases = []
        for case in smooth_f64_cases():
            cli.write(a, f64hex(case["pos"]))
            cli.write(b, f64hex(case["vel"]))
            cli.write(c, f64hex(case["target"]))
            bufs = cli.call("x2game.dll+b0ad0",
                            [a, b, case["dt"], c, case["response"]],
                            [(a, 24), (b, 24)])
            out_cases.append({"in": case, "pos": f64hex(hexfloats64(bufs[0])),
                              "vel": f64hex(hexfloats64(bufs[1]))})
        (GOLD / "smooth_f64.json").write_text(json.dumps(out_cases, indent=1))
        print("f64 cases", len(out_cases))

        # Constants used by the sample clamp / turn smoothing, read from the module image.
        consts = {}
        for name, rva, ty in [
            ("dt_max", 0x9b3040, "f32"), ("dt_min", 0x9acf30, "f32"),
            ("half", 0x9ac47c, "f32"), ("one", 0x9ac488, "f32"),
            ("turn_min", 0x9acf90, "f32"), ("turn_max", 0x9acce8, "f32"),
            ("poly_a", 0x9b2998, "f32"), ("poly_b", 0x9b2994, "f32"),
        ]:
            raw = cli.run("peek-global", f"0x{base + rva:x}", ty)
            consts[name] = raw.split("value=")[-1].strip() if "value=" in raw else raw
        print(consts)
        (GOLD / "constants.json").write_text(json.dumps({"peek": consts}, indent=1))
    finally:
        d.close()


if __name__ == "__main__":
    main()
