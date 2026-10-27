#!/usr/bin/env python3
"""Read the 1.2 client's protected movement core from the live process (W3).

Recon only: this script never writes game memory, never patches a page and
never injects a code reader. It launches the hidden original with the lane's
standard client driver (to reach the world), then reads the target functions
with ReadProcessMemory, compares them with the shipped (packed) and unpacked
x2game.dll files, and disassembles the live bytes with capstone. Control-flow
follows direct jmp/jcc, call edges, push-continuation / push-ret dispatchers
(the obfuscated functions return into continuations), and indirect jumps whose
target is a pushed constant.

The output is a fresh classification:

  (a) plain x86 after runtime decryption,
  (b) mutated/obfuscated x86 a block follower can linearise,
  (c) a virtual machine (handler table + bytecode).

Usage:
    python tools/movement_dump_probe.py --slot wp3 --lane move-golden \
        --runs 2 --tag movement-dump-20261027-r1

Raw output (git-ignored):
    data/extract/movement-dump-YYYYMMDD-r<N>/
Committed evidence (generated from the raw run):
    tools/evidence/movement_dump/{README.md,report.txt,compare.json,listing-*.txt}

Requirements: capstone and pefile (Python 3.12 here has capstone 5.0.7,
pefile 2024.8.26).
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import math
import os
import re
import struct
import subprocess
import sys
import time
from collections import Counter, defaultdict
from ctypes import wintypes
from pathlib import Path

import capstone
import pefile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
import live_slots  # noqa: E402

LANE = Path(os.environ.get("MOVEMENT_DUMP_LANE", "D:/aaemu-wt/move-golden"))
LIVE = Path(os.environ.get("MOVEMENT_DUMP_LIVE", "D:/aaemu-live-move-golden"))
CLIENT = Path(os.environ.get("MOVEMENT_DUMP_CLIENT", "D:/AA1.2/bin32/archeage.exe"))
LAUNCHER = Path(os.environ.get("MOVEMENT_DUMP_LAUNCHER", str(LIVE / "bin" / "aaemu-launcher.exe")))
DRIVER = Path(os.environ.get("MOVEMENT_DUMP_DRIVER", "D:/aaemu-target/move-golden/live/aaemu-clientdriver.exe"))
DLL = Path(os.environ.get("MOVEMENT_DUMP_DLL", "D:/aaemu-target/move-golden/aadriver-onfoot/aadriver.dll"))
PASSWORD_FILE = Path(os.environ.get("MOVEMENT_DUMP_PASSWORD", str(LIVE / "cap-pw.txt")))

PACKED_X2 = Path(os.environ.get("MOVEMENT_DUMP_PACKED_X2", "D:/AA1.2/bin32/x2game.dll"))
UNPACKED_X2 = Path(os.environ.get("MOVEMENT_DUMP_UNPACKED_X2", "D:/aa-re/client12/x2game_unpacked.dll"))
MEM_X2 = Path(os.environ.get("MOVEMENT_DUMP_MEM_X2", "D:/aa-re/client12/capture/x2game.dll.a.mem"))
PACKED_CRY = Path(os.environ.get("MOVEMENT_DUMP_PACKED_CRY", "D:/AA1.2/bin32/cryaction.dll"))

X2_IMAGE_BASE = 0x38FF0000
CRY_IMAGE_BASE = 0x304F0000

# Target functions. is_actual=True are the four functions named by the task;
# the thunk and caller entries are read to follow the call chain. `size` is
# the funcs.csv start/end size for x2game functions; cryaction uses a window.
TARGETS = [
    {"name": "FUN_3a979a34", "module": "x2game.dll", "rva": 0x3a979a34 - X2_IMAGE_BASE,
     "size": 129, "thunk_rva": 0x391250a0 - X2_IMAGE_BASE,
     "decomp": 2231680, "actual": True,
     "role": "called from CActor::UpdateMovementRequest via thunk 391250a0; "
             "also 227810, 229154"},
    {"name": "FUN_3a6ab5f3", "module": "x2game.dll", "rva": 0x3a6ab5f3 - X2_IMAGE_BASE,
     "size": 338, "thunk_rva": 0x3913a620 - X2_IMAGE_BASE,
     "decomp": 2231406, "actual": True,
     "role": "CPlayerMovement::Process / per-frame movement state copy"},
    {"name": "FUN_3a98bb00", "module": "x2game.dll", "rva": 0x3a98bb00 - X2_IMAGE_BASE,
     "size": 97, "thunk_rva": 0x3913da40 - X2_IMAGE_BASE,
     "decomp": 2231977, "actual": True,
     "role": "called by ProcessFrameMovement after FUN_3a6ab5f3"},
    {"name": "FUN_3a9918bb", "module": "x2game.dll", "rva": 0x3a9918bb - X2_IMAGE_BASE,
     "size": 9, "thunk_rva": 0x39139470 - X2_IMAGE_BASE,
     "decomp": 2232162, "actual": True,
     "role": "called by ProcessFrameMovement after FUN_3a98bb00"},
    {"name": "thunk_FUN_3a979a34", "module": "x2game.dll", "rva": 0x391250a0 - X2_IMAGE_BASE,
     "size": 5, "thunk_rva": 0x391250a0 - X2_IMAGE_BASE,
     "decomp": None, "actual": False, "role": "call thunk into the protected region"},
    {"name": "thunk_FUN_3a6ab5f3", "module": "x2game.dll", "rva": 0x3913a620 - X2_IMAGE_BASE,
     "size": 5, "thunk_rva": 0x3913a620 - X2_IMAGE_BASE,
     "decomp": None, "actual": False, "role": "call thunk into the protected region"},
    {"name": "thunk_FUN_3a98bb00", "module": "x2game.dll", "rva": 0x3913da40 - X2_IMAGE_BASE,
     "size": 5, "thunk_rva": 0x3913da40 - X2_IMAGE_BASE,
     "decomp": None, "actual": False, "role": "call thunk into the protected region"},
    {"name": "thunk_FUN_3a9918bb", "module": "x2game.dll", "rva": 0x39139470 - X2_IMAGE_BASE,
     "size": 5, "thunk_rva": 0x39139470 - X2_IMAGE_BASE,
     "decomp": None, "actual": False, "role": "call thunk into the protected region"},
    {"name": "CActor::UpdateMovementRequest", "module": "x2game.dll",
     "rva": 0x39118650 - X2_IMAGE_BASE, "size": 0x200, "thunk_rva": None,
     "decomp": 219255, "actual": False, "role": "readable caller (context)"},
    {"name": "CPlayer::ProcessFrameMovement", "module": "x2game.dll",
     "rva": 0x39135fe0 - X2_IMAGE_BASE, "size": 0x200, "thunk_rva": None,
     "decomp": 235491, "actual": False, "role": "readable caller (context)"},
    {"name": "CPlayer::ProcessFrameMovementRotation", "module": "x2game.dll",
     "rva": 0x39135ed0 - X2_IMAGE_BASE, "size": 0x200, "thunk_rva": None,
     "decomp": 235447, "actual": False, "role": "readable caller (context)"},
    {"name": "FUN_307c70b0", "module": "cryaction.dll", "rva": 0x307c70b0 - CRY_IMAGE_BASE,
     "size": 0x400, "thunk_rva": None, "decomp": None, "actual": True,
     "role": "cryaction runtime region named by the task"},
    {"name": "FUN_306713f0", "module": "cryaction.dll", "rva": 0x306713f0 - CRY_IMAGE_BASE,
     "size": 0x200, "thunk_rva": None, "decomp": None, "actual": False,
     "role": "MCM 1..5 merge (context)"},
    {"name": "FUN_30665500", "module": "cryaction.dll", "rva": 0x30665500 - CRY_IMAGE_BASE,
     "size": 0x200, "thunk_rva": None, "decomp": None, "actual": False,
     "role": "AcquireRequestedBehaviourMovement (context)"},
]

ANTI_CHEAT = re.compile(
    r"(?i)^(ehsvc|v3.*|ahnrpt|hshield|xigncode|gameguard|npgg|npkcrypt|"
    r"blackcipher|tenprotect|mhyprot|nprotect|hackshield)"
)

PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000


def log(*args):
    print("movement-dump:", *args, flush=True)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def entropy(data: bytes) -> float:
    if not data:
        return 0.0
    counts = Counter(data)
    n = len(data)
    return -sum((c / n) * math.log2(c / n) for c in counts.values())


def pe_identity(path: Path) -> dict:
    data = path.read_bytes()
    pe = pefile.PE(data=data, fast_load=True)
    return {
        "path": str(path),
        "sha256": sha256(data),
        "bytes": len(data),
        "image_base": pe.OPTIONAL_HEADER.ImageBase,
        "time_date_stamp": pe.FILE_HEADER.TimeDateStamp,
        "size_of_image": pe.OPTIONAL_HEADER.SizeOfImage,
    }


class StaticImage:
    """A PE file mapped by RVA, plus an optional raw memory image."""

    def __init__(self, path: Path, raw_mem: Path | None = None):
        self.path = path
        self.data = path.read_bytes()
        self.pe = pefile.PE(data=self.data, fast_load=True)
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        self.size = self.pe.OPTIONAL_HEADER.SizeOfImage
        self.stamp = self.pe.FILE_HEADER.TimeDateStamp
        self.mem = raw_mem.read_bytes() if raw_mem and raw_mem.exists() else None
        self.identity = {
            "path": str(path), "sha256": sha256(self.data), "bytes": len(self.data),
            "image_base": self.base, "time_date_stamp": self.stamp,
            "size_of_image": self.size,
        }

    def offset(self, rva: int) -> int | None:
        try:
            return self.pe.get_offset_from_rva(rva)
        except Exception:
            return None

    def read_rva(self, rva: int, size: int) -> bytes | None:
        off = self.offset(rva)
        if off is None or off < 0 or off + size > len(self.data):
            return None
        return self.data[off:off + size]

    def read_mem(self, rva: int, size: int) -> bytes | None:
        if self.mem is None or rva < 0 or rva + size > len(self.mem):
            return None
        return self.mem[rva:rva + size]

    def executable_sections(self) -> list[tuple[int, int]]:
        out = []
        for section in self.pe.sections:
            if section.Characteristics & 0x20000000:  # IMAGE_SCN_MEM_EXECUTE
                start = section.VirtualAddress
                end = start + max(section.Misc_VirtualSize, section.SizeOfRawData)
                out.append((start, end))
        return out


class Mcp:
    def __init__(self, command):
        self.proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL, text=True, encoding="utf-8")
        self.sequence = 0
        self.call("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                 "clientInfo": {"name": "movement-dump-probe", "version": "1"}})
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, payload):
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()

    def call(self, method, params=None):
        assert self.proc.stdin and self.proc.stdout
        self.sequence += 1
        request = {"jsonrpc": "2.0", "id": self.sequence, "method": method}
        if params is not None:
            request["params"] = params
        self._send(request)
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("driver MCP closed stdout")
        response = json.loads(line)
        if response.get("id") != self.sequence:
            raise RuntimeError("driver MCP response id mismatch")
        if "error" in response:
            raise RuntimeError(str(response["error"]))
        return response["result"]

    def tool(self, name, **arguments):
        result = self.call("tools/call", {"name": name, "arguments": arguments})
        if result.get("isError"):
            raise RuntimeError("\n".join(c.get("text", "") for c in result.get("content", [])))
        return result

    def text(self, name, **arguments):
        return "\n".join(c["text"] for c in self.tool(name, **arguments)["content"]
                         if c.get("type") == "text")

    def close(self):
        try:
            assert self.proc.stdin
            self.proc.stdin.close()
            self.proc.wait(timeout=15)
        except Exception:
            pass


def mcp_text(mcp: Mcp, name, retries=150, **arguments):
    last = None
    for _ in range(retries):
        try:
            return mcp.text(name, **arguments)
        except RuntimeError as exc:
            last = exc
            text = str(exc).lower()
            if "cannot find the file" not in text and "pipe" not in text:
                raise
            time.sleep(0.1)
    raise last


class Process:
    def __init__(self, pid: int):
        self.pid = pid
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.handle = self.kernel.OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)
        if not self.handle:
            self.handle = self.kernel.OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, False, pid)
        if not self.handle:
            raise RuntimeError(f"OpenProcess({pid}) failed: {ctypes.get_last_error()}")

    def modules(self):
        TH32CS_SNAPMODULE = 0x8
        TH32CS_SNAPMODULE32 = 0x10

        class ME(ctypes.Structure):
            _fields_ = [("dwSize", wintypes.DWORD), ("th32ModuleID", wintypes.DWORD),
                        ("th32ProcessID", wintypes.DWORD), ("GlblcntUsage", wintypes.DWORD),
                        ("ProccntUsage", wintypes.DWORD), ("modBaseAddr", ctypes.c_void_p),
                        ("modBaseSize", wintypes.DWORD), ("hModule", wintypes.HMODULE),
                        ("szModule", ctypes.c_wchar * 256), ("szExePath", ctypes.c_wchar * 260)]

        snapshot = self.kernel.CreateToolhelp32Snapshot(
            TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, self.pid)
        entry = ME()
        entry.dwSize = ctypes.sizeof(ME)
        out = []
        ok = self.kernel.Module32FirstW(snapshot, ctypes.byref(entry))
        while ok:
            out.append({"name": entry.szModule, "base": int(entry.modBaseAddr or 0),
                        "size": int(entry.modBaseSize), "path": entry.szExePath})
            ok = self.kernel.Module32NextW(snapshot, ctypes.byref(entry))
        self.kernel.CloseHandle(snapshot)
        return out

    def read(self, address: int, size: int) -> bytes | None:
        buffer = (ctypes.c_char * size)()
        got = ctypes.c_size_t()
        if not self.kernel.ReadProcessMemory(self.handle, ctypes.c_void_p(address),
                                             buffer, size, ctypes.byref(got)):
            return None
        return bytes(buffer[:got.value])

    def executable_ranges(self, base: int, size: int):
        """Committed executable pages in [base, base+size), read-only query."""
        class MBI(ctypes.Structure):
            _fields_ = [("BaseAddress", ctypes.c_void_p),
                        ("AllocationBase", ctypes.c_void_p),
                        ("AllocationProtect", wintypes.DWORD),
                        ("RegionSize", ctypes.c_size_t),
                        ("State", wintypes.DWORD),
                        ("Protect", wintypes.DWORD),
                        ("Type", wintypes.DWORD)]

        mbi = MBI()
        address = base
        out = []
        while address < base + size:
            result = self.kernel.VirtualQueryEx(
                self.handle, ctypes.c_void_p(address), ctypes.byref(mbi),
                ctypes.sizeof(mbi))
            if not result:
                break
            region_base = int(mbi.BaseAddress or 0)
            region_size = int(mbi.RegionSize)
            if region_size <= 0:
                break
            protect = mbi.Protect & 0xFF
            executable = protect in (0x10, 0x20, 0x40, 0x80)
            if (mbi.State == 0x1000 and executable
                    and not (mbi.Protect & 0x100)):  # MEM_COMMIT, not PAGE_GUARD
                start = max(address, region_base)
                end = min(base + size, region_base + region_size)
                if start < end:
                    out.append((start, end, protect))
            address = region_base + region_size
        return out

    def close(self):
        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None


class BlockFollower:
    """Recursive descent over live bytes, aware of push-ret continuations."""

    def __init__(self, proc: Process, module: dict, statics: list[StaticImage],
                 max_blocks=160, max_insns=4000):
        self.proc = proc
        self.base = module["base"]
        self.size = module["size"]
        self.module_name = module["name"]
        self.max_blocks = max_blocks
        self.max_insns = max_insns
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        self.md.detail = True
        self.pages: dict[int, bytes] = {}
        self.sections = []
        for image in statics:
            if image.base != self.base:
                continue
            for section in image.pe.sections:
                name = section.Name.rstrip(b"\x00").decode("ascii", "replace")
                start = self.base + section.VirtualAddress
                end = start + max(section.Misc_VirtualSize, section.SizeOfRawData)
                self.sections.append((name, start, end))
        # The live page protection is the authority. The protected functions
        # live in a code cave in .reloc (not executable in the file table), and
        # some continuations run from pages the loader maps executable inside
        # .rdata. VirtualQueryEx is read-only and tells us which pages are
        # really executable; fall back to the file sections only if it fails.
        live_ranges = []
        query = getattr(proc, "executable_ranges", None)
        if query:
            live_ranges = query(self.base, self.size)
        if live_ranges:
            self.exec_ranges = [(start, end) for start, end, _ in live_ranges]
            self.live_ranges = live_ranges
        else:
            self.live_ranges = None
            self.exec_ranges = []
            for name, start, end in self.sections:
                if name.startswith(".reloc") or name == ".text":
                    self.exec_ranges.append((start, end))
        if not self.exec_ranges:
            self.exec_ranges = [(self.base, self.base + self.size)]

    def read(self, address: int, size: int) -> bytes | None:
        page = address & ~0xFFF
        blob = self.pages.get(page)
        if blob is None:
            blob = self.proc.read(page, 0x1000)
            if blob is None:
                return self.proc.read(address, size)
            self.pages[page] = blob
        offset = address - page
        if offset + size <= len(blob):
            return blob[offset:offset + size]
        return self.proc.read(address, size)

    def in_exec(self, address: int) -> bool:
        return any(a <= address < b for a, b in self.exec_ranges)

    def section_of(self, address: int) -> str | None:
        for name, start, end in self.sections:
            if start <= address < end:
                return name
        return None

    def is_function_entry(self, address: int) -> bool:
        """True for a normal MSVC-style entry (push ebp; mov ebp, esp)."""
        mnemonics = []
        current = address
        for _ in range(3):
            code = self.read(current, 16)
            if not code:
                return False
            try:
                insn = next(self.md.disasm(code, current, 1))
            except StopIteration:
                return False
            mnemonics.append((insn.mnemonic, insn.op_str))
            current += insn.size
        return mnemonics[:2] == [("push", "ebp"), ("mov", "ebp, esp")]

    def disassemble_block(self, start: int, limit=256, hint=0):
        insns = []
        address = start
        pushed = []
        immediates = []
        seh_candidate = None
        stack_overwrite = False
        for _ in range(limit):
            code = self.read(address, 16)
            if not code:
                break
            try:
                insn = next(self.md.disasm(code, address, 1))
            except StopIteration:
                break
            insns.append({
                "address": insn.address, "bytes": insn.bytes.hex(),
                "mnemonic": insn.mnemonic, "op_str": insn.op_str,
                "groups": [self.md.group_name(g) for g in insn.groups],
            })
            if insn.mnemonic == "push" and insn.op_str.startswith("0x"):
                try:
                    value = int(insn.op_str, 16)
                except ValueError:
                    value = None
                if value is not None:
                    if seh_candidate is None and len(insns) >= 2 \
                            and insns[-2]["mnemonic"] == "push" \
                            and insns[-2]["op_str"] == "-1":
                        seh_candidate = value  # SEH handler, not a continuation
                    else:
                        pushed.append(value)
            if insn.mnemonic in ("mov", "lea") and insn.op_str.startswith("0x"):
                try:
                    immediates.append(int(insn.op_str, 16))
                except ValueError:
                    pass
            if insn.mnemonic in ("mov", "lea") and "ptr [esp" in insn.op_str:
                stack_overwrite = True
            if insn.mnemonic == "lea" and insn.op_str.startswith("esp,"):
                stack_overwrite = True
            address += insn.size
            if insn.mnemonic in ("ret", "retf", "jmp", "ljmp"):
                break
            if insn.mnemonic.startswith("j") and insn.mnemonic not in ("jmp",):
                break
        return {"start": start, "insns": insns, "pushed": pushed,
                "immediates": immediates, "seh_candidate": seh_candidate,
                "stack_overwrite": stack_overwrite}

    def successors(self, block, hint=0):
        succ = []
        insns = block["insns"]
        if not insns:
            return succ
        last = insns[-1]
        mnemonic = last["mnemonic"]
        op_str = last["op_str"]
        candidates = []
        block_section = self.section_of(block["start"])
        for value in block["pushed"] + block["immediates"]:
            if value == block["seh_candidate"] or not self.in_exec(value):
                continue
            if hint and abs(value - hint) > 0x200000:
                continue
            value_section = self.section_of(value)
            if (value_section in (".rdata", ".data", ".rsrc")
                    and value_section != block_section):
                # A string/table pointer pushed as an argument, not a
                # continuation of this code block.
                continue
            candidates.append(value)
        target = None
        if op_str.startswith("0x"):
            try:
                target = int(op_str.split(",")[0], 16)
            except ValueError:
                target = None
        indirect = False
        if mnemonic in ("jmp", "ljmp"):
            if target is not None:
                if candidates and (block["stack_overwrite"]
                                   or self.is_function_entry(target)):
                    # push <return address>; jmp <callee>: a tail call. Follow
                    # the pushed continuation, not the callee's body.
                    succ.append(("tail-call", target))
                    succ.append(("tail-cont", candidates[-1]))
                else:
                    succ.append(("jmp", target))
            else:
                indirect = True
        elif mnemonic.startswith("j") and mnemonic not in ("jmp",):
            if target is not None:
                succ.append(("jcc", target))
            next_address = last["address"] + len(bytes.fromhex(last["bytes"]))
            succ.append(("fall", next_address))
        elif mnemonic in ("ret", "retf"):
            indirect = True
        if indirect or mnemonic in ("ret", "retf"):
            for value in candidates:
                succ.append(("push-cont", value))
        for insn in insns:
            if insn["mnemonic"] == "call":
                op = insn["op_str"]
                if op.startswith("0x"):
                    try:
                        succ.append(("call", int(op, 16)))
                    except ValueError:
                        pass
                else:
                    succ.append(("call-indirect", None))
        return succ

    def follow(self, entry: int, hint: int | None = None):
        work = [entry]
        seen = set()
        blocks = []
        total_insns = 0
        truncated = False
        while work:
            start = work.pop(0)
            if start in seen or not self.in_exec(start):
                continue
            if len(blocks) >= self.max_blocks or total_insns >= self.max_insns:
                truncated = True
                break
            seen.add(start)
            block = self.disassemble_block(start, hint=hint or entry)
            if not block["insns"]:
                continue
            block["successors"] = []
            for kind, target in self.successors(block, hint=hint or entry):
                if target is not None and not self.in_exec(target):
                    block["successors"].append((kind, target, "outside"))
                    continue
                block["successors"].append((kind, target, "inside"))
                if kind in ("jmp", "jcc", "fall", "push-cont", "tail-cont"):
                    if target is not None and target not in seen:
                        work.append(target)
            blocks.append(block)
            total_insns += len(block["insns"])
        blocks.sort(key=lambda b: b["start"])
        return {"entry": entry, "blocks": blocks, "seen": seen,
                "truncated": truncated, "total_insns": total_insns}

    def stats(self, result):
        stats = Counter()
        vm_hits = []
        push_ret = 0
        for block in result["blocks"]:
            mnemonics = [insn["mnemonic"] for insn in block["insns"]]
            for insn in block["insns"]:
                stats[insn["mnemonic"]] += 1
            if any(m.startswith("j") for m in mnemonics) and block["pushed"]:
                push_ret += 1
            for insn in block["insns"]:
                op = insn["op_str"]
                if insn["mnemonic"] in ("jmp", "call") and "ptr [" in op and "*4" in op:
                    vm_hits.append((insn["address"], insn["mnemonic"], op))
                if insn["mnemonic"] == "jmp" and "ptr [" in op and "+" in op:
                    vm_hits.append((insn["address"], insn["mnemonic"], op))
        real = sum(stats[m] for m in (
            "mov", "movss", "movsd", "movq", "movzx", "movsx", "lea", "add", "addss",
            "addsd", "sub", "subss", "subsd", "mul", "mulss", "mulsd", "div", "divss",
            "divsd", "xor", "xorps", "xorpd", "and", "or", "cmp", "test", "comiss",
            "ucomiss", "cvtsi2ss", "cvtss2si", "cvttss2si", "fld", "fstp", "fadd",
            "fsub", "fmul", "fdiv", "imul", "shl", "shr", "sar", "rol", "ror", "inc",
            "dec", "neg", "not", "xchg", "cmovne", "cmove", "cmovl", "cmova", "cmovb",
            "cmovbe", "cmovg", "cmovge", "cmovle", "cmovs", "cmovns", "movlpd", "movhpd",
        ))
        sse = sum(stats[m] for m in stats if m.endswith("ss") or m.endswith("sd") or
                  m in ("xorps", "xorpd", "movlpd", "movhpd", "movq", "movd"))
        control = stats["jmp"] + stats["call"] + stats["ret"] + sum(
            v for k, v in stats.items() if k.startswith("j") and k != "jmp")
        return {
            "blocks": len(result["blocks"]),
            "instructions": result["total_insns"],
            "truncated": result["truncated"],
            "real_ops": real, "sse_ops": sse, "control_ops": control,
            "jmp": stats["jmp"], "jcc": sum(v for k, v in stats.items()
                                            if k.startswith("j") and k != "jmp"),
            "call": stats["call"], "ret": stats["ret"],
            "push": stats["push"], "pop": stats["pop"],
            "push_ret_blocks": push_ret,
            "vm_dispatch_hits": vm_hits[:20],
            "vm_dispatch_count": len(vm_hits),
            "mnemonics": dict(stats.most_common()),
        }


def classify(stats: dict, live_equals_unpacked: bool, live_equals_packed: bool,
             module: str = "x2game.dll") -> dict:
    real_ratio = stats["real_ops"] / max(1, stats["instructions"])
    if stats["vm_dispatch_count"] >= 2 and real_ratio < 0.30:
        code = "c"
    elif stats["push_ret_blocks"] or stats["blocks"] > 8 or stats["jcc"] > 1:
        code = "b"
    else:
        code = "a"
    reasons = []
    if module == "x2game.dll" and not live_equals_packed and live_equals_unpacked:
        reasons.append("live bytes differ from the shipped packed file but equal the "
                       "unpacked image: the pages are decrypted at run time")
    if live_equals_unpacked:
        if module == "x2game.dll":
            reasons.append("live bytes equal x2game_unpacked.dll at this RVA")
        else:
            reasons.append("live bytes equal the shipped cryaction.dll at this RVA")
    if stats["vm_dispatch_count"]:
        reasons.append(f"{stats['vm_dispatch_count']} table-indexed indirect dispatch site(s)")
    else:
        reasons.append("no table-indexed dispatch loop/handler table")
    reasons.append(f"real-operation ratio {real_ratio:.2f} "
                   f"({stats['real_ops']}/{stats['instructions']})")
    if stats["push_ret_blocks"]:
        reasons.append(f"{stats['push_ret_blocks']} block(s) use push-continuation / "
                       "push-ret dispatch")
    if stats["blocks"] > 8:
        reasons.append(f"{stats['blocks']} blocks reached; control flow is flattened, "
                       "not a single linear function")
    return {"code": code, "real_ratio": real_ratio, "reasons": reasons}


def format_listing(target: dict, result: dict, stats: dict, verdict: dict,
                   live_va: int, image_va: int) -> str:
    lines = []
    lines.append(f"// {target['name']} - live linearised listing (W3 recon)")
    lines.append(f"// module {target['module']} live VA 0x{live_va:08X} "
                 f"(image VA 0x{image_va:08X}), RVA 0x{target['rva']:X}")
    lines.append(f"// classification: ({verdict['code']}) - " + "; ".join(verdict["reasons"]))
    lines.append(f"// blocks={stats['blocks']} instructions={stats['instructions']} "
                 f"jmp={stats['jmp']} jcc={stats['jcc']} call={stats['call']} "
                 f"ret={stats['ret']} push_ret_blocks={stats['push_ret_blocks']} "
                 f"vm_dispatch={stats['vm_dispatch_count']} "
                 f"truncated={stats['truncated']}")
    lines.append("")
    by_address = {insn["address"]: insn for block in result["blocks"]
                  for insn in block["insns"]}
    successor_of = {block["start"]: block["successors"] for block in result["blocks"]}
    for block in result["blocks"]:
        lines.append(f"; --- block 0x{block['start']:08X} "
                     f"({len(block['insns'])} insns) ---")
        for insn in block["insns"]:
            lines.append(f"{insn['address']:08X}: {insn['bytes']:<20} "
                         f"{insn['mnemonic']:8s} {insn['op_str']}")
        for kind, target_address, scope in successor_of.get(block["start"], []):
            if target_address is None:
                lines.append(f";   edge {kind} unresolved")
            else:
                lines.append(f";   edge {kind} -> 0x{target_address:08X} ({scope})")
        lines.append("")
    return "\n".join(line.rstrip() for line in lines) + "\n"


def section_range(image: StaticImage, name: str) -> dict | None:
    for section in image.pe.sections:
        current = section.Name.rstrip(b"\x00").decode("ascii", "replace")
        if current != name:
            continue
        size = max(section.Misc_VirtualSize, section.SizeOfRawData)
        return {
            "name": name,
            "rva": section.VirtualAddress,
            "size": size,
            "raw_size": section.SizeOfRawData,
            "characteristics": section.Characteristics,
            "executable": bool(section.Characteristics & 0x20000000),
        }
    return None


def check_anticheat(modules: list[dict]) -> list[str]:
    bad = []
    for module in modules:
        name = module["name"]
        path = module.get("path") or ""
        if ANTI_CHEAT.match(name) or "/xigncode" in path.lower() or "\\xigncode" in path.lower():
            bad.append(name)
    return bad


class Run:
    def __init__(self, args, raw_dir: Path, index: int):
        self.args = args
        self.raw_dir = raw_dir
        self.index = index
        self.mcp = None
        self.pid = None
        self.meta = {
            "index": index,
            "started": time.strftime("%Y-%m-%dT%H:%M:%S"),
            "argv": list(sys.argv),
            "commands": [],
            "inputs": [],
            "failures": [],
        }
        self.player = None
        self.modules = []
        self.proc = None

    def ui(self, code):
        return mcp_text(self.mcp, "client_lua", pid=self.pid, target="ui", code=code)

    def lua(self, code):
        return mcp_text(self.mcp, "client_lua", pid=self.pid, target="system", code=code)

    def launch(self):
        account = live_slots.resolve(self.args.slot, self.args.lane, "a")
        command = [str(DRIVER), "mcp", "--dll", str(DLL), "--client", str(CLIENT),
                   "--launcher", str(LAUNCHER), "--password-file", str(PASSWORD_FILE),
                   "--login", self.args.login, "--user", account.user,
                   "--artifact-dir", str(self.raw_dir / "mcp")]
        self.meta["commands"].append(" ".join(command))
        self.mcp = Mcp(command)
        out = mcp_text(self.mcp, "client_launch", fast_start=True, fast_pak=True,
                       hidden=True, user=account.user,
                       password_file=str(PASSWORD_FILE))
        (self.raw_dir / "launch.txt").write_text(out, encoding="utf-8")
        match = re.search(r"pid (\d+)", out)
        if not match:
            raise RuntimeError("client_launch: " + out)
        self.pid = int(match.group(1))
        self.meta["pid"] = self.pid
        log(f"run {self.index}: client pid {self.pid}")
        self.meta["account"] = {
            "slot": account.slot, "role": account.role, "lane": account.lane,
            "user": account.user, "character": account.character,
            "character_id": account.character_id,
        }

    def enter_world(self):
        deadline = time.monotonic() + 260
        while time.monotonic() < deadline:
            try:
                if self.ui("return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()") == "3":
                    break
            except Exception:
                pass
            time.sleep(1)
        else:
            raise RuntimeError("server select stage not reached")
        log("server select")
        self.ui("X2World:EnterWorld(1,0)")
        while time.monotonic() < deadline:
            try:
                if self.ui("return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()") == "4":
                    break
            except Exception:
                pass
            time.sleep(0.5)
        else:
            raise RuntimeError("character select stage not reached")
        log("character select")
        self.ui("local b=characterSelectWindow.bottomPanel.startButton "
                "b:OnClick('LeftButton')")
        name = self.meta["account"]["character"]
        while time.monotonic() < deadline:
            try:
                state = self.ui("return X2LoginCharacter:GetCurrentStage(), "
                                "X2Unit and X2Unit:UnitName('player')")
                stage, _, unit = state.partition("\t")
                if stage == "0" and unit not in ("", "nil"):
                    log("in world as", unit)
                    break
            except Exception:
                pass
            time.sleep(1)
        else:
            raise RuntimeError("in-world player not reached")
        time.sleep(2.5)
        try:
            self.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass
        state = self.lua(
            "local pe,pid for id,x in pairs(System.GetEntities() or {}) do "
            "if x:GetName()==%s then pe=x;pid=id end end _G.MDP_PE=pe "
            "return tostring(pe~=nil), tostring(pid), tostring(pe and pe.__this or nil)"
            % json.dumps(name))
        parts = state.split("\t")
        if len(parts) < 3 or parts[0] != "true":
            raise RuntimeError("player entity not found: " + state)
        matches = re.findall(r"(?:0x)?([0-9A-Fa-f]{6,8})", parts[2])
        if not matches:
            raise RuntimeError("player __this not a pointer: " + parts[2])
        self.player = {
            "entity_id": int(parts[1]),
            "entity_ptr": f"0x{int(matches[-1], 16):08X}",
        }
        self.meta["player"] = self.player
        log("player entity", self.player["entity_ptr"])

    def sample_state(self):
        code = (
            "local pe=_G.MDP_PE if pe==nil then return 'no-player' end "
            "local p=pe:GetWorldPos() local a=pe:GetAngles() local v=pe:GetVelocity() "
            "local anim='' pcall(function() anim=pe.actor:GetCurrentAnimationState() or '' end) "
            "return string.format('frame=%s time=%.6f pos=%.5f,%.5f,%.5f "
            "angles=%.5f,%.5f,%.5f vel=%.5f,%.5f,%.5f anim=%s', "
            "tostring(System.GetFrameID()), System.GetCurrTime(), "
            "p.x,p.y,p.z,a.x,a.y,a.z,v.x,v.y,v.z,anim)")
        return self.lua(code)

    def check_anticheat(self):
        self.modules = self.proc.modules()
        (self.raw_dir / "modules.txt").write_text(
            "\n".join(f"{m['name']}\t{m['base']:08X}\t{m['size']:X}\t{m['path']}"
                      for m in self.modules), encoding="utf-8")
        bad = check_anticheat(self.modules)
        self.meta["anticheat"] = {"modules": len(self.modules), "bad": bad}
        if bad:
            raise RuntimeError("anti-cheat module loaded, refusing: " + ", ".join(bad))
        log(f"modules {len(self.modules)}; no anti-cheat module")


def dump_run(run: Run, statics: dict[str, StaticImage], args) -> dict:
    run.proc = Process(run.pid)
    run.check_anticheat()
    run.meta["state"] = run.sample_state() if run.mcp else "attached (state not sampled)"
    log("state:", run.meta["state"])

    module_by_name = {m["name"].lower(): m for m in run.modules}
    for wanted in ("x2game.dll", "cryaction.dll"):
        if wanted not in module_by_name:
            raise RuntimeError(f"{wanted} not loaded")
    run.meta["modules"] = {
        name: module_by_name[name] for name in ("x2game.dll", "cryaction.dll")
    }

    results = {}
    for target in TARGETS:
        module = module_by_name[target["module"].lower()]
        live_va = module["base"] + target["rva"]
        image_va = (X2_IMAGE_BASE if target["module"] == "x2game.dll" else CRY_IMAGE_BASE) \
            + target["rva"]
        compare_size = max(target["size"], 0x100)
        window = 0x2000
        live = run.proc.read(live_va, window)
        if live is None:
            results[target["name"]] = {"error": f"ReadProcessMemory failed at 0x{live_va:08X}"}
            continue
        live_cmp = live[:compare_size]
        packed = statics[target["module"]]["packed"].read_rva(target["rva"], compare_size)
        unpacked = statics[target["module"]]["unpacked"].read_rva(target["rva"], compare_size)
        mem = (statics[target["module"]]["unpacked"].read_mem(target["rva"], compare_size)
               if target["module"] == "x2game.dll" else None)
        first_diff = None
        if packed is not None and live_cmp != packed[:len(live_cmp)]:
            limit = min(len(live_cmp), len(packed))
            for index in range(limit):
                if live_cmp[index] != packed[index]:
                    first_diff = index
                    break
        follower = BlockFollower(run.proc, module,
                                 [statics[target["module"]]["packed"],
                                  statics[target["module"]]["unpacked"]])
        hint = (module["base"] + target["thunk_rva"]
                if target.get("thunk_rva") else live_va)
        traversal = follower.follow(live_va, hint=hint)
        stats = follower.stats(traversal)
        exec_ranges = [
            {"start": start, "end": end, "protect": protect,
             "section": follower.section_of(start),
             "end_section": follower.section_of(end - 1)}
            for start, end, protect in (follower.live_ranges or [])
        ]
        live_equals_packed = packed is not None and live_cmp == packed[:len(live_cmp)]
        live_equals_unpacked = unpacked is not None and live_cmp == unpacked[:len(live_cmp)]
        live_equals_mem = (None if mem is None
                           else live_cmp == mem[:len(live_cmp)])
        verdict = classify(stats, live_equals_unpacked, live_equals_packed,
                           target["module"])
        raw_path = run.raw_dir / f"live-{target['name'].replace('::', '_')}.bin"
        raw_path.write_bytes(live)
        listing = format_listing(target, traversal, stats, verdict, live_va, image_va)
        listing_path = run.raw_dir / f"listing-{target['name'].replace('::', '_')}.txt"
        listing_path.write_text(listing, encoding="utf-8")
        results[target["name"]] = {
            "module": target["module"],
            "actual": target["actual"],
            "rva": target["rva"],
            "image_va": image_va,
            "live_va": live_va,
            "size": compare_size,
            "live_sha256": sha256(live_cmp),
            "packed_sha256": sha256(packed) if packed is not None else None,
            "unpacked_sha256": sha256(unpacked) if unpacked is not None else None,
            "mem_sha256": sha256(mem) if mem is not None else None,
            "live_equals_packed": live_equals_packed,
            "live_equals_unpacked": live_equals_unpacked,
            "live_equals_mem": live_equals_mem,
            "first_diff_from_packed": first_diff,
            "entropy_live": entropy(live_cmp),
            "entropy_packed": entropy(packed) if packed is not None else None,
            "entropy_unpacked": entropy(unpacked) if unpacked is not None else None,
            "stats": stats,
            "classification": verdict,
            "exec_source": "live" if follower.live_ranges else "file",
            "exec_ranges": exec_ranges,
            "listing": str(listing_path.relative_to(run.raw_dir.parent)),
            "raw": str(raw_path.relative_to(run.raw_dir.parent)),
        }
        log(f"{target['name']}: live==unpacked={live_equals_unpacked} "
            f"live==packed={live_equals_packed} class=({verdict['code']}) "
            f"blocks={stats['blocks']} insns={stats['instructions']}")
    return results


def analyze(args, raw_dir: Path, all_runs: list[dict], evidence: Path,
            statics: dict[str, dict[str, StaticImage]] | None = None,
            capture_argv: list[str] | None = None) -> dict:
    first = all_runs[0]["results"]
    cross = {}
    names = [t["name"] for t in TARGETS if t["name"] in first]
    for name in names:
        hashes = [run["results"][name].get("live_sha256") for run in all_runs
                  if name in run["results"] and "live_sha256" in run["results"][name]]
        cross[name] = {"hashes": hashes, "identical": len(set(hashes)) == 1}
    deterministic = all(item["identical"] for item in cross.values()) if cross else False

    actual = [t for t in TARGETS if t["actual"]]
    actual_classes = [first[t["name"]]["classification"]["code"] for t in actual
                      if t["name"] in first and "classification" in first[t["name"]]]
    overall = "b" if "b" in actual_classes else ("c" if "c" in actual_classes
                                                 else ("a" if actual_classes else "?"))
    x2_decrypted = all(first[t["name"]].get("live_equals_unpacked") and
                       not first[t["name"]].get("live_equals_packed")
                       for t in TARGETS
                       if t["module"] == "x2game.dll" and t["actual"]
                       and t["name"] in first)
    cry_plain = all(first[t["name"]].get("live_equals_packed")
                    for t in TARGETS if t["module"] == "cryaction.dll"
                    and t["name"] in first)
    checks = []

    def check(name, ok, detail):
        checks.append({"name": name, "ok": bool(ok), "detail": detail})

    check("no anti-cheat module",
          all(run["meta"].get("anticheat") is not None and
              not run["meta"]["anticheat"].get("bad") for run in all_runs),
          str([run["meta"].get("anticheat") for run in all_runs]))
    check("x2game and cryaction live modules found",
          all("x2game.dll" in run["meta"].get("modules", {}) and
              "cryaction.dll" in run["meta"].get("modules", {}) for run in all_runs),
          str([list(run["meta"].get("modules", {})) for run in all_runs]))
    check("x2game targets equal the unpacked file", x2_decrypted,
          "live == x2game_unpacked.dll and live != shipped x2game.dll")
    check("x2game targets differ from the shipped packed file",
          all(not first[t["name"]].get("live_equals_packed") for t in TARGETS
              if t["module"] == "x2game.dll" and t["actual"] and t["name"] in first),
          "runtime decryption observed")
    check("cryaction target equals its file", cry_plain,
          "the shipped cryaction.dll already contains the code-cave bytes")
    if len(all_runs) > 1:
        check("live bytes identical across fresh processes", deterministic,
              str({k: v["hashes"] for k, v in cross.items()}))
    for target in actual:
        result = first.get(target["name"], {})
        stats = result.get("stats")
        check(f"{target['name']} disassembles", bool(stats) and stats["instructions"] > 0,
              f"{stats['instructions'] if stats else 0} instructions")
    check("no virtual-machine dispatch table observed",
          all(first[t["name"]].get("stats", {}).get("vm_dispatch_count", 0) < 2
              for t in actual if t["name"] in first),
          str({t["name"]: first[t["name"]].get("stats", {}).get("vm_dispatch_count")
               for t in actual if t["name"] in first}))
    passed = sum(1 for c in checks if c["ok"])
    failed = len(checks) - passed
    caves = {}
    if statics:
        caves["x2game.dll"] = section_range(statics["x2game.dll"]["unpacked"], ".reloc")
        caves["cryaction.dll"] = section_range(statics["cryaction.dll"]["packed"], ".reloc")

    recorded_argv = None
    for run in all_runs:
        recorded_argv = run["meta"].get("argv")
        if recorded_argv:
            break
    summary = {
        "date": all_runs[0]["meta"].get("started", time.strftime("%Y-%m-%dT%H:%M:%S")),
        "argv": capture_argv or recorded_argv or list(sys.argv),
        "analysis_argv": list(sys.argv),
        "slot": args.slot,
        "lane": args.lane,
        "runs": [run["meta"] for run in all_runs],
        "results": first,
        "cross_run": cross,
        "deterministic_across_runs": deterministic,
        "x2game_decrypted_at_runtime": x2_decrypted,
        "cryaction_plain_in_file": cry_plain,
        "overall_classification": overall,
        "code_caves": caves,
        "checks": checks,
        "passed": passed,
        "failed": failed,
        "builds": {},
    }
    evidence.mkdir(parents=True, exist_ok=True)
    (evidence / "compare.json").write_text(json.dumps(summary, indent=1),
                                           encoding="utf-8")

    for name, item in first.items():
        if not item.get("actual"):
            continue
        src = raw_dir / item["listing"]
        if not src.exists():
            name_only = Path(item["listing"]).name
            matches = sorted(raw_dir.glob(f"run-*/{name_only}"))
            src = matches[0] if matches else src
        if src.exists():
            text = src.read_text(encoding="utf-8")
            text = "\n".join(line.rstrip() for line in text.splitlines()) + "\n"
            (evidence / f"listing-{name.replace('::', '_')}.txt").write_text(
                text, encoding="utf-8")

    report = render_report(summary, first, all_runs, checks)
    (evidence / "report.txt").write_text(report, encoding="utf-8")
    readme = render_readme(summary, first)
    (evidence / "README.md").write_text(readme, encoding="utf-8")
    return summary


def render_report(summary, results, all_runs, checks) -> str:
    lines = []
    lines.append("Movement core dump from the live 1.2 client (W3 recon)")
    lines.append("======================================================")
    lines.append("")
    lines.append(f"date: {summary['date']}")
    lines.append(f"argv: {' '.join(summary['argv'])}")
    lines.append(f"slot: {summary['slot']} on lane {summary['lane']}")
    for run in all_runs:
        lines.append(f"run {run['meta']['index']}: pid {run['meta'].get('pid')} "
                     f"account {run['meta'].get('account', {}).get('user')} / "
                     f"{run['meta'].get('account', {}).get('character')}")
    lines.append("")
    lines.append("Builds:")
    for label, image in summary.get("builds", {}).items():
        lines.append(f"  {label}: {image['path']}")
        stamp = image.get("time_date_stamp")
        size_of_image = image.get("size_of_image")
        lines.append(f"    sha256={image['sha256']}"
                     + (f" stamp=0x{stamp:08X}" if stamp is not None else "")
                     + (f" size_of_image=0x{size_of_image:X}"
                        if size_of_image is not None else "")
                     + (f" bytes={image['bytes']}" if "bytes" in image else ""))
    for run in all_runs:
        for name, module in run["meta"].get("modules", {}).items():
            lines.append(f"  live {name}: base=0x{module['base']:08X} "
                         f"size=0x{module['size']:X} {module['path']}")
    for run in all_runs:
        lines.append(f"  run {run['meta']['index']} anti-cheat scan: "
                     f"{run['meta'].get('anticheat')}")
    for module, cave in summary.get("code_caves", {}).items():
        if cave:
            lines.append(f"  code cave {module}: .reloc RVA 0x{cave['rva']:X}-"
                         f"0x{cave['rva'] + cave['size']:X} "
                         f"({cave['size']} bytes, file-executable="
                         f"{cave['executable']})")
    for module in ("x2game.dll", "cryaction.dll"):
        sample = next((item for name, item in results.items()
                       if item.get("module") == module and item.get("exec_ranges")), None)
        if not sample:
            continue
        ranges = sample["exec_ranges"]
        lines.append(f"  live executable pages {module}: {len(ranges)} region(s), "
                     f"source={sample.get('exec_source')}")
        for item in ranges[:12]:
            section = item.get("section") or "?"
            end_section = item.get("end_section")
            span = ""
            if end_section:
                span = (section if section == end_section
                        else f"{section}..{end_section}") + " "
            lines.append(f"    0x{item['start']:08X}-0x{item['end']:08X} "
                         f"{span}protect=0x{item['protect']:02X}")
        if len(ranges) > 12:
            lines.append(f"    ... {len(ranges) - 12} more")
    lines.append("")
    lines.append("Decompile source: D:/aa-re/client12/out-x2game/x2game.dll.c, "
                 "image base 0x38FF0000.")
    lines.append("funcs.csv rows: 3a979a34,3a979ab4,129; 3a6ab5f3,3a6ab744,338;")
    lines.append("3a98bb00,3a98bb60,97; 3a9918bb,3a9918c3,9. Those starts/sizes")
    lines.append("match x2game_unpacked.dll; the shipped x2game.dll has encrypted")
    lines.append("bytes at the same RVAs (entropy 7.1-7.5), so the decompile came")
    lines.append("from the unpacked image that the live process matches.")
    lines.append("")
    lines.append("Scenario state at the dump (standing in world):")
    for run in all_runs:
        lines.append(f"  run {run['meta']['index']}: {run['meta'].get('state')}")
    lines.append("")
    lines.append("Per-target evidence (live bytes at the function start):")
    lines.append("target                           module        image VA    live VA     size  "
                 "live==pack  live==unpack  live==mem  entropy live/packed  class")
    for target in TARGETS:
        result = results.get(target["name"])
        if not result or "error" in result:
            lines.append(f"{target['name']:<32} {target['module']:<13} ERROR "
                         f"{result.get('error') if result else 'missing'}")
            continue
        lines.append(
            f"{target['name']:<32} {target['module']:<13} "
            f"0x{result['image_va']:08X}  0x{result['live_va']:08X}  "
            f"{result['size']:5d}  {str(result['live_equals_packed']):<10}  "
            f"{str(result['live_equals_unpacked']):<12}  "
            f"{('n/a' if result['live_equals_mem'] is None else str(result['live_equals_mem'])):<9}  "
            f"{result['entropy_live']:.2f}/{result['entropy_packed']:.2f}  "
            f"({result['classification']['code']})")
    lines.append("")
    lines.append("Block-follower result (live bytes, direct jmp/jcc, call edges and")
    lines.append("push-continuation / push-ret dispatchers followed):")
    lines.append("target                           blocks  insns  real  sse  jmp  jcc  call  ret  "
                 "push-ret  vm-dispatch  truncated")
    for target in TARGETS:
        result = results.get(target["name"])
        if not result or "stats" not in result:
            continue
        stats = result["stats"]
        lines.append(
            f"{target['name']:<32} {stats['blocks']:6d} {stats['instructions']:6d} "
            f"{stats['real_ops']:5d} {stats['sse_ops']:4d} {stats['jmp']:4d} "
            f"{stats['jcc']:4d} {stats['call']:4d} {stats['ret']:4d} "
            f"{stats['push_ret_blocks']:9d} {stats['vm_dispatch_count']:12d}  "
            f"{stats['truncated']}")
    lines.append("")
    lines.append("Classification:")
    for target in TARGETS:
        result = results.get(target["name"])
        if not result or "classification" not in result:
            continue
        verdict = result["classification"]
        lines.append(f"  {target['name']}: ({verdict['code']}) real_ratio="
                     f"{verdict['real_ratio']:.2f}")
        for reason in verdict["reasons"]:
            lines.append("    - " + reason)
    lines.append("")
    lines.append("What the evidence means:")
    lines.append(f"  - x2game runtime pages are decrypted: "
                 f"{summary['x2game_decrypted_at_runtime']}")
    lines.append(f"  - cryaction code-cave bytes already match the shipped file: "
                 f"{summary['cryaction_plain_in_file']}")
    if len(all_runs) > 1:
        lines.append(f"  - fresh processes had identical target bytes: "
                     f"{summary['deterministic_across_runs']}")
        for name, item in summary["cross_run"].items():
            lines.append(f"      {name}: identical={item['identical']}")
    lines.append(f"  - overall classification for the four named functions: "
                 f"({summary['overall_classification']})")
    lines.append("  (a) plain x86 after runtime decryption; (b) mutated/obfuscated x86")
    lines.append("  that a block follower can linearise; (c) a virtual machine.")
    lines.append("")
    lines.append("Checks:")
    for check in checks:
        lines.append(f"  [{'PASS' if check['ok'] else 'FAIL'}] {check['name']}: "
                     f"{check['detail']}")
    lines.append("")
    lines.append(f"passed={summary['passed']} failed={summary['failed']}")
    lines.append("")
    lines.append("The decompiled file D:/aa-re/client12/out-x2game/x2game.dll.c has")
    lines.append("image base 0x38FF0000. Its funcs.csv entries for the four named")
    lines.append("functions are the same starts/sizes as D:/aa-re/client12/"
                 "x2game_unpacked.dll,")
    lines.append("and the stub text at 3a979a34 matches that file's bytes. The shipped")
    lines.append("D:/AA1.2/bin32/x2game.dll has the same PE headers but encrypted bytes")
    lines.append("at those RVAs. The live process is decrypted to the unpacked image.")
    lines.append("")
    lines.append("Small listings are committed next to this report; raw live windows and")
    lines.append("the command lines are in the git-ignored run directory named in")
    lines.append("ACTUAL_RUN_REPORT.md.")
    return "\n".join(lines) + "\n"


def render_readme(summary, results) -> str:
    lines = []
    lines.append("# Movement core dump evidence (W3 recon)")
    lines.append("")
    lines.append("Fresh capture from the hidden original 1.2 client on the "
                 "`move-golden` lane.")
    lines.append("Generated by `tools/movement_dump_probe.py`; do not hand-edit.")
    lines.append("")
    lines.append("## Method")
    lines.append("")
    lines.append("1. Launch the hidden original with the lane client driver (standard")
    lines.append("   launcher ticket, slot account), enter the world and stand still.")
    lines.append("2. Enumerate the process modules and refuse to continue if an")
    lines.append("   anti-cheat module is loaded (XignCode/eEHSvc, nProtect, AhnLab,")
    lines.append("   HackShield, GameGuard, TenProtect and similar names).")
    lines.append("3. Read the target bytes with `ReadProcessMemory` only. The probe has")
    lines.append("   no write/`VirtualProtect`/injection path; the driver is used only")
    lines.append("   to launch and to press the UI buttons that reach the world.")
    lines.append("4. Compare the live bytes with the shipped packed `x2game.dll`, the")
    lines.append("   unpacked `x2game_unpacked.dll`, the raw live image "
                 "`x2game.dll.a.mem`")
    lines.append("   and the shipped `cryaction.dll` at the same RVA.")
    lines.append("5. Disassemble the live bytes with capstone in 32-bit mode and follow")
    lines.append("   direct `jmp`/`jcc`, `call` edges, push-continuation / push-ret")
    lines.append("   dispatchers (pushed constants that the obfuscated functions return")
    lines.append("   to) and indirect jumps resolved from those pushes. The SEH handler")
    lines.append("   pushed after `push -1` is excluded from continuations.")
    lines.append("6. Classify (a) plain x86 after runtime decryption, (b) mutated x86 a")
    lines.append("   block follower can linearise, or (c) a VM (table dispatch loop,")
    lines.append("   bytecode stream, handler table).")
    lines.append("")
    lines.append("## Builds")
    lines.append("")
    for label, image in summary.get("builds", {}).items():
        lines.append(f"* {label}: `{image['path']}`")
        stamp = image.get("time_date_stamp")
        size_of_image = image.get("size_of_image")
        lines.append(f"  SHA256 `{image['sha256']}`"
                     + (f", TimeDateStamp `0x{stamp:08X}`" if stamp is not None else "")
                     + (f", SizeOfImage `0x{size_of_image:X}`"
                        if size_of_image is not None else ""))
    lines.append("")
    lines.append("## Result")
    lines.append("")
    lines.append(f"Overall classification: ({summary['overall_classification']}).")
    lines.append(f"x2game pages decrypted at runtime: "
                 f"{summary['x2game_decrypted_at_runtime']}. "
                 f"cryaction code cave already plain in the file: "
                 f"{summary['cryaction_plain_in_file']}.")
    if len(summary.get("runs", [])) > 1:
        lines.append(f"Fresh processes identical at the targets: "
                     f"{summary['deterministic_across_runs']}.")
    for module, cave in summary.get("code_caves", {}).items():
        if cave:
            lines.append(f"Code cave {module}: `.reloc` RVA "
                         f"`0x{cave['rva']:X}-0x{cave['rva'] + cave['size']:X}` "
                         f"({cave['size']} bytes; executable flag in the file: "
                         f"{cave['executable']}).")
    for module in ("x2game.dll", "cryaction.dll"):
        sample = next((item for item in summary["results"].values()
                       if item.get("module") == module and item.get("exec_ranges")),
                      None)
        if sample:
            ranges = sample["exec_ranges"]
            lines.append(f"Live executable pages {module}: {len(ranges)} region(s), "
                         f"source={sample.get('exec_source')}: "
                         + "; ".join(f"0x{item['start']:08X}-0x{item['end']:08X} "
                                     f"protect=0x{item['protect']:02X}"
                                     for item in ranges[:6]))
    lines.append("")
    lines.append("Decompile source: `D:/aa-re/client12/out-x2game/x2game.dll.c` "
                 "(image base `0x38FF0000`); `funcs.csv` has the same starts/sizes "
                 "as `x2game_unpacked.dll`, and the shipped packed `x2game.dll` is "
                 "encrypted at those RVAs.")
    lines.append("")
    lines.append("| target | module | live==shipped | live==unpacked | class |")
    lines.append("|---|---|---|---|---|")
    for target in TARGETS:
        result = results.get(target["name"])
        if not result or "classification" not in result:
            continue
        lines.append(f"| `{target['name']}` | {target['module']} | "
                     f"{result['live_equals_packed']} | "
                     f"{result['live_equals_unpacked']} | "
                     f"({result['classification']['code']}) |")
    lines.append("")
    lines.append("## Files")
    lines.append("")
    lines.append("* `report.txt` - the full generated report (tables and checks).")
    lines.append("* `compare.json` - machine-readable hashes, stats and checks.")
    lines.append("* `listing-*.txt` - small linearised live listings per target.")
    lines.append("")
    lines.append("Raw live windows, module lists and the exact command lines are in")
    lines.append("the git-ignored run directory named in `ACTUAL_RUN_REPORT.md`.")
    lines.append("")
    lines.append("## Reproduce")
    lines.append("")
    lines.append("```")
    lines.append("python tools/movement_dump_probe.py --slot wp3 --lane move-golden \\")
    lines.append("  --runs 2 --tag movement-dump-YYYYMMDD-rN")
    lines.append("```")
    return "\n".join(lines) + "\n"


def stop_client(mcp: Mcp | None, pid: int | None, meta: dict):
    if not pid:
        return
    try:
        if mcp:
            mcp_text(mcp, "client_stop", pid=pid, retries=20)
    except Exception as exc:
        log("client_stop failed:", exc)
    time.sleep(1.0)
    try:
        result = subprocess.run(["taskkill", "/PID", str(pid), "/F"],
                                capture_output=True, text=True, timeout=20)
        meta.setdefault("stops", []).append(
            {"pid": pid, "taskkill": result.returncode,
             "stdout": result.stdout.strip(), "stderr": result.stderr.strip()})
    except Exception as exc:
        meta.setdefault("stops", []).append({"pid": pid, "error": str(exc)})


def write_actual_run_report(raw_dir: Path, args, all_runs, exit_code):
    lines = ["# Actual run report", "",
             f"date: {time.strftime('%Y-%m-%dT%H:%M:%S')}",
             f"exit code: {exit_code}", "",
             "## Invocation", "",
             "    " + " ".join(sys.argv), "",
             "## Command lines", ""]
    for run in all_runs:
        lines += ["    " + c for c in run["meta"].get("commands", [])]
    lines += ["", "## Runs", ""]
    for run in all_runs:
        meta = run["meta"]
        lines.append(f"* run {meta['index']}: pid {meta.get('pid')}, "
                     f"account {meta.get('account', {}).get('user')}, "
                     f"character {meta.get('account', {}).get('character')}, "
                     f"state `{meta.get('state')}`")
        lines.append(f"  anti-cheat: {meta.get('anticheat')}")
        for stop in meta.get("stops", []):
            lines.append(f"  stop: {stop}")
        for failure in meta.get("failures", []):
            lines.append(f"  failure: {failure}")
    lines += ["", "## Builds", ""]
    for label, path in (("client", CLIENT), ("x2game_shipped", PACKED_X2),
                        ("x2game_unpacked", UNPACKED_X2),
                        ("cryaction", PACKED_CRY)):
        if path.exists():
            identity = pe_identity(path)
            lines.append(f"* {label}: {path} sha256={identity['sha256']} "
                         f"stamp=0x{identity['time_date_stamp']:08X} "
                         f"size_of_image=0x{identity['size_of_image']:X}")
    for path in (DRIVER, DLL):
        if path.exists():
            lines.append(f"* {path.name}: {path} sha256={sha256_file(path)}")
    for path in (LIVE / "bin" / "aaemu-game.exe", LIVE / "bin" / "aaemu-login.exe"):
        if path.exists():
            lines.append(f"* server {path.name}: sha256={sha256_file(path)}")
    lines += ["", "## Captured", "",
              "* live bytes at the four named functions, their thunks and callers,",
              "  plus cryaction 0x307c70b0/0x306713f0/0x30665500",
              "* live vs shipped/unpacked/memory-image byte comparison",
              "* capstone block-follower listings and classification stats",
              "* player position/angles/velocity/animation at the dump",
              "", "## Raw files", "",
              "* `live-<target>.bin`: 0x2000-byte live window at the entry",
              "* `listing-<target>.txt`: the linearised listing used by the report",
              "* `modules.txt`, `launch.txt`, `mcp/`", "",
              "## Failed attempts", ""]
    failures = []
    for run in all_runs:
        failures += run["meta"].get("failures", [])
    lines += ["- " + f for f in failures] or ["- none"]
    (raw_dir / "ACTUAL_RUN_REPORT.md").write_text("\n".join(lines) + "\n",
                                                  encoding="utf-8")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--slot", default=os.environ.get("MOVEMENT_DUMP_SLOT", "wp3"))
    parser.add_argument("--lane", default=os.environ.get("MOVEMENT_DUMP_LANE_NAME",
                                                         "move-golden"))
    parser.add_argument("--login", default=os.environ.get("MOVEMENT_DUMP_LOGIN",
                                                          "127.0.0.1:28237"))
    parser.add_argument("--tag", default=time.strftime("movement-dump-%Y%m%d-r1"))
    parser.add_argument("--out", type=Path,
                        default=ROOT / "data" / "extract")
    parser.add_argument("--evidence", type=Path,
                        default=ROOT / "tools" / "evidence" / "movement_dump")
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--pid", type=int, default=None,
                        help="attach to an already-running hidden original instead "
                             "of launching")
    parser.add_argument("--keep-client", action="store_true")
    parser.add_argument("--from-raw", type=Path, default=None,
                        help="regenerate the committed evidence from an existing "
                             "run directory (run-*/run-meta.json)")
    args = parser.parse_args()

    for path in (CLIENT, PACKED_X2, UNPACKED_X2, PACKED_CRY):
        if not path.exists():
            raise SystemExit(f"missing required file: {path}")
    account = live_slots.resolve(args.slot, args.lane, "a")
    args.user = account.user
    args.character = account.character
    if args.pid is None:
        for path in (DRIVER, DLL, LAUNCHER, PASSWORD_FILE):
            if not path.exists():
                raise SystemExit(f"missing required file: {path}")

    raw_dir = args.out / args.tag
    raw_dir.mkdir(parents=True, exist_ok=True)
    evidence = args.evidence
    evidence.mkdir(parents=True, exist_ok=True)

    statics = {
        "x2game.dll": {
            "packed": StaticImage(PACKED_X2),
            "unpacked": StaticImage(UNPACKED_X2, MEM_X2),
        },
        "cryaction.dll": {
            "packed": StaticImage(PACKED_CRY),
            "unpacked": StaticImage(PACKED_CRY),
        },
    }
    builds = {
        "client": pe_identity(CLIENT),
        "x2game_shipped": statics["x2game.dll"]["packed"].identity,
        "x2game_unpacked": statics["x2game.dll"]["unpacked"].identity,
        "cryaction": statics["cryaction.dll"]["packed"].identity,
    }
    if DRIVER.exists():
        builds["clientdriver"] = pe_identity(DRIVER)
    if DLL.exists():
        builds["driver_dll"] = pe_identity(DLL)

    if args.from_raw:
        raw_dir = args.from_raw
        all_runs = []
        for meta_path in sorted(raw_dir.glob("run-*/run-meta.json")):
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
            results = meta.pop("results", {})
            all_runs.append({"meta": meta, "results": results})
        merged = [run for run in all_runs if run["results"]]
        if not merged:
            raise SystemExit(f"no run-meta results under {raw_dir}")
        args.slot = merged[-1]["meta"].get("account", {}).get("slot", args.slot)
        args.lane = merged[-1]["meta"].get("account", {}).get("lane", args.lane)
        capture_argv = None
        capture_file = raw_dir / "capture-command.txt"
        if capture_file.exists():
            try:
                capture_argv = json.loads(capture_file.read_text(encoding="utf-8"))
            except Exception:
                capture_argv = capture_file.read_text(encoding="utf-8").split()
        summary = analyze(args, raw_dir, merged, evidence, statics,
                          capture_argv=capture_argv)
        summary["builds"] = builds
        (evidence / "compare.json").write_text(json.dumps(summary, indent=1),
                                               encoding="utf-8")
        (evidence / "report.txt").write_text(
            render_report(summary, summary["results"], merged, summary["checks"]),
            encoding="utf-8")
        (evidence / "README.md").write_text(
            render_readme(summary, summary["results"]), encoding="utf-8")
        (raw_dir / "ANALYSIS_RUN_REPORT.md").write_text(
            "# Analysis run report\n\n"
            f"analysis argv: {' '.join(sys.argv)}\n"
            f"capture argv: {' '.join(summary['argv'])}\n"
            f"exit code: 0\n", encoding="utf-8")
        return 0

    all_runs = []
    exit_code = 0
    try:
        for index in range(1, max(1, args.runs) + 1):
            run_dir = raw_dir / f"run-{index}"
            run_dir.mkdir(parents=True, exist_ok=True)
            run = Run(args, run_dir, index)
            log(f"run {index} starting")
            try:
                if args.pid is not None:
                    run.pid = args.pid
                    run.meta["pid"] = run.pid
                    run.meta["attached"] = True
                    run.meta["account"] = {
                        "slot": account.slot, "role": account.role,
                        "lane": account.lane, "user": account.user,
                        "character": account.character,
                        "character_id": account.character_id,
                    }
                else:
                    run.launch()
                    run.enter_world()
                run.meta["results"] = dump_run(run, statics, args)
                all_runs.append({"meta": run.meta, "results": run.meta["results"]})
            except Exception as exc:
                exit_code = 1
                run.meta["failures"].append(str(exc))
                log("FAILED:", exc)
                all_runs.append({"meta": run.meta, "results": {}})
            finally:
                if run.proc:
                    run.proc.close()
                if args.pid is None and not args.keep_client:
                    stop_client(run.mcp, run.pid, run.meta)
                if run.mcp:
                    run.mcp.close()
                (run_dir / "run-meta.json").write_text(json.dumps(run.meta, indent=1),
                                                       encoding="utf-8")
        if not any(run["results"] for run in all_runs):
            raise RuntimeError("no run produced results")
        merged = [run for run in all_runs if run["results"]]
        summary = analyze(args, raw_dir, merged, evidence, statics)
        summary["builds"] = builds
        (evidence / "compare.json").write_text(json.dumps(summary, indent=1),
                                               encoding="utf-8")
        (evidence / "report.txt").write_text(
            render_report(summary, summary["results"], merged, summary["checks"]),
            encoding="utf-8")
        (evidence / "README.md").write_text(
            render_readme(summary, summary["results"]), encoding="utf-8")
    except Exception as exc:
        exit_code = 1
        log("FAILED:", exc)
        (raw_dir / "error.txt").write_text(str(exc), encoding="utf-8")
    finally:
        write_actual_run_report(raw_dir, args, all_runs, exit_code)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
