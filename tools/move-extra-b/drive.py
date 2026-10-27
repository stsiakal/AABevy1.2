#!/usr/bin/env python3
"""Tiny driver wrapper for the hidden 1.2 client (aaemu-clientdriver CLI)."""
import os
import struct
import subprocess
import sys

DRIVER = r"D:/aaemu-target/veh-golden/live/aaemu-clientdriver.exe"
DLL = r"D:/aaemu-target/veh-golden/aadriver/aadriver.dll"
PID = os.environ.get("AA_PID", "63720")
BASE = 0x38FF0000
ENV = dict(os.environ, AADRIVER_DLL=DLL)


def run(*args):
    p = subprocess.run([DRIVER, *args, "--pid", PID], capture_output=True, text=True, env=ENV)
    if p.returncode != 0:
        raise RuntimeError(p.stderr.strip() or p.stdout.strip())
    return p.stdout.strip()


def rva(va):
    return va - BASE


def alloc(n):
    return int(run("alloc", str(n)), 16)


def read(addr, n):
    return bytes.fromhex(run("read", hex(addr), str(n)))


def write(addr, data, unsafe=False):
    args = ["write", hex(addr), data.hex()]
    if unsafe:
        args.append("--unsafe")
    return run(*args)


def peek(addr, ty="ptr"):
    return int(run("peek-global", hex(addr), ty).split()[-1])


def call(va, conv, this=None, args=(), dumps=(), unsafe=False):
    spec = [f"x2game.dll+0x{rva(va):x}", conv]
    if this is not None:
        spec.append(f"this={hex(this)}")
    for d in dumps:
        spec.append(f"dump={hex(d[0])}:{d[1]}")
    if unsafe:
        spec.append("unsafe")
    spec += [str(a) for a in args]
    return run("call", *spec)


def call_f32(va, conv, this=None, args=(), dumps=()):
    out = call(va, conv, this, args, dumps)
    vals = {}
    for line in out.splitlines():
        if line.startswith("st0_f32="):
            vals["st0"] = float(line.split("=", 1)[1])
        elif line.startswith("eax="):
            vals["eax"] = int(line.split("=", 1)[1], 16)
        elif line.startswith("dump "):
            pass
    return out


def lua(code, target="ui"):
    return run("lua", "-t", target, code)


def chat(text):
    code = (
        'g_msgs={} X2Chat.DispatchChatMessage=function(...) local t={...} '
        'local s="" for i=1,table.maxn(t) do s=s..tostring(t[i]).."|" end '
        'table.insert(g_msgs,s) end X2Chat:SendChatMessage(CHAT_SAY, ' + lua_quote(text) + ', "")'
    )
    lua(code)
    import time
    time.sleep(2)
    return lua('return table.concat(g_msgs, "\\n")')


def lua_quote(text):
    return '"' + "".join(f"\\{b:03d}" for b in text.encode("utf-8")) + '"'


def parse_call(out):
    """Return (eax, st0, {dump_name: bytes}) from `call` output."""
    eax = None
    st0 = None
    dumps = {}
    for line in out.splitlines():
        if line.startswith("eax=0x"):
            eax = int(line.split("=", 1)[1], 16)
        elif line.startswith("st0="):
            try:
                st0 = float(line.split("=", 1)[1])
            except ValueError:
                st0 = None
        elif line.startswith("dump "):
            # dump 0x....:1024 = hex
            head, _, hexdata = line.partition(" = ")
            dumps[head] = bytes.fromhex(hexdata.strip())
    return eax, st0, dumps


if __name__ == "__main__":
    print(run(*sys.argv[1:]))
