# The protected on-foot movement core in the running 1.2 client (W3)

Recon, 2026-10-27, time-boxed. Fresh read-only capture of the hidden original
1.2 client on the `move-golden` lane, slot `wp3` (`wp3a` / `MgWp3a`), standing
in the world at `4365.03125, 4389.68359, 231.20627`, animation
`fist_relaxed_idle+fist_ba_relaxed_idle`. Two fresh processes were read: pids
14372 and 47444. The lane stack, client binaries and the driver are pinned in
`tools/evidence/movement_dump/report.txt`.

## Scope and sources

* Probe: `tools/movement_dump_probe.py` (committed).
* Generated evidence: `tools/evidence/movement_dump/` - `README.md`,
  `report.txt`, `compare.json`, and small linearised listings
  `listing-FUN_3a979a34.txt`, `listing-FUN_3a6ab5f3.txt`,
  `listing-FUN_3a98bb00.txt`, `listing-FUN_3a9918bb.txt`,
  `listing-FUN_307c70b0.txt`.
* Raw run (git-ignored): `data/extract/movement-dump-20261027-r4/`
  (`run-1/`, `run-2/`, `ACTUAL_RUN_REPORT.md`, live windows, module lists,
  MCP launch output).
* Client `archeage.exe` SHA256 `a6fbd9ff58bf749fd995489f34acb01e1bc84d4d5a533721b0c017562766fc2a`.
  Shipped `x2game.dll` SHA256 `3821c34366ff8b6f7aed95ba53d12216a5c31cb7c3407703d28eea639a621205`,
  TimeDateStamp `0x543CB835`, SizeOfImage `0x1CB0A00`.
  Unpacked `D:/aa-re/client12/x2game_unpacked.dll` SHA256
  `5ae3cad0dd4b40ee3da09fdb573bd8d58f4d8a3a3776ef87b456eecbed02ef1d`.
  `cryaction.dll` SHA256 `1c0ca9a7f7aab883029b64adb6af4bdc76043fe2cf173f7fc9b0d1d7f62b0399`.

The probe launches with the lane's normal hidden client harness (the driver is
used only to launch and to press the UI buttons that reach the world). The
module list is enumerated first and the probe refuses to continue if an
anti-cheat module is present; 157 modules were loaded in both runs and none
matched. All target reads use `ReadProcessMemory`; the probe has no write,
`VirtualProtect`, or code-injection path.

## Method

1. Read the target RVAs from the live `x2game.dll` / `cryaction.dll`.
2. Compare the live bytes with the shipped packed `x2game.dll`, the unpacked
   `x2game_unpacked.dll`, the old raw image `x2game.dll.a.mem`, and the
   shipped `cryaction.dll` at the same RVA.
3. Disassemble the live bytes with capstone in 32-bit mode. Follow direct
   `jmp`/`jcc`, `call` edges, and the push-continuation / push-ret dispatchers
   used by the obfuscated functions: a block that pushes a return address and
   then jumps (or returns through `jmp [esp-4]`) continues at the pushed
   address. The SEH handler pushed after `push -1` is excluded. Call targets
   that are normal function entries are recorded as tail calls and not
   inlined.
4. Classify (a) plain x86 after runtime decryption, (b) mutated/obfuscated x86
   that a block follower can linearise, or (c) a VM (dispatch loop, handler
   table, bytecode).

## [R] What the fresh run proves

* **x2game is decrypted at run time.** At every target the live bytes equal
  `x2game_unpacked.dll` over the compared windows (`FUN_3a979a34` 256 bytes,
  `FUN_3a6ab5f3` 338, `FUN_3a98bb00` 256, `FUN_3a9918bb` 256, the four
  thunks and the three caller entries), and differ from the shipped
  `x2game.dll` at the same RVAs. Live entropy is 5.2-6.0 versus 7.1-7.5 in
  the shipped file. The shipped bytes do not disassemble; the live bytes do.
* **The result is deterministic across processes.** Both fresh pids produced
  identical SHA256 for every target window
  (`compare.json`, `cross_run.*.identical = true`). The older
  `x2game.dll.a.mem` also matches the fresh live bytes at all x2game targets;
  it is corroboration, not the authority.
* **cryaction needs no runtime decryption for this target.** The live bytes at
  `0x307C70B0` equal the shipped `cryaction.dll`; the code cave is already
  plain in the file. Its `.reloc` section is RVA `0x2B8000-0x548A00` and is
  not marked executable in the file; the x2game code cave is `.reloc` RVA
  `0x1652000-0x1CB0A00`, also not marked executable in the file.
* **The live pages are RWX, not just `.reloc`.** `VirtualQueryEx` (read-only)
  reports x2game `0x38FF1000-0x3ACA0A00` with protect `0x40`
  (`PAGE_EXECUTE_READWRITE`) and the header page with `0x80`; cryaction
  `0x304F1000-0x30A38A00` with `0x40`. The loader maps the whole image RWX,
  so page protection alone cannot separate code from data. The follower
  therefore refuses a pushed `.rdata`/`.data`/`.rsrc` pointer from a
  `.text`/`.reloc` block (a string argument) but still follows a direct
  `jmp` into `.rdata` code, e.g. `39AC11CB` from `3912510F`.
* **The decompile source is the unpacked image.** `out-x2game/x2game.dll.c`
  uses image base `0x38FF0000`; its `funcs.csv` rows for the named functions
  are `3a979a34,3a979ab4,129`, `3a6ab5f3,3a6ab744,338`,
  `3a98bb00,3a98bb60,97`, `3a9918bb,3a9918c3,9`, which are the starts/sizes
  in `x2game_unpacked.dll`. The shipped `x2game.dll` has the same headers but
  encrypted bytes there, so the decompile did not come from the shipped file.
* **The reachable code is real x86, not bytecode.** The block follower reaches
  20 blocks / 100 instructions at `FUN_3a979a34`, 3 / 91 at `FUN_3a6ab5f3`,
  160 / 617 at `FUN_3a98bb00` (the 160-block cap was reached, so the listing is
  truncated), 11 / 32 at `FUN_3a9918bb`, and 35 / 151 at `FUN_307c70b0`.
  Real-operation ratios are 0.63, 0.86, 0.52, 0.38 and 0.57;
  `FUN_3a6ab5f3` alone has 21 SSE operations and `FUN_3a98bb00` 154. The
  listings show the original frame being manipulated directly:
  * `FUN_3a979a34` (`listing-FUN_3a979a34.txt`): the decompiled 129-byte stub
    is only the SEH frame install and `ret` at `3A979A34-3A979AB4`
    (lines 83-120); the real body is reached through the pushed continuations
    `391250E3` (line 7) / `3912510F` (line 15) and blocks at `3913D490`
    (line 31), `39AC11CB` (line 47), `3A6A0CAC` (line 73), etc.
  * `FUN_3a6ab5f3` (`listing-FUN_3a6ab5f3.txt`): a state copy. It fills a
    struct from the actor at `+0x2e0`, `+0x2e8`, `+0x96c`, `+0x974`, `+0x9dc`,
    `+0x284`, `+0x161`, `+0x78/0x79` and copies 13 dwords (lines 54-95), then
    does `push 0x3913a753; jmp edx` (lines 101-102) and the epilogue continues
    at `39123657`.
  * `FUN_3a98bb00` (`listing-FUN_3a98bb00.txt`): vector arithmetic, including
    `addss`/`mulss` sequences at lines 66-69, 287-294, 416-417 and 428-429
    (the block-level meaning is not yet labelled; the listing stops at the cap).
  * `FUN_3a9918bb` (`listing-FUN_3a9918bb.txt`): a 9-byte thunk into a
    flattened block at `39125632`.
  * `FUN_307c70b0` (`listing-FUN_307c70b0.txt`): the same SEH + push-ret
    shape in cryaction; the entry's real continuations are `30667993` and
    `306679C0`.
* **No table-indexed dispatch loop was found at the named targets.** The four
  x2game targets and `FUN_307c70b0` have zero table-dispatch sites in the
  traversed blocks. The only table jumps in the context functions are normal
  compiler switch tables (`FUN_306713f0`: 6 sites, real ratio 0.75;
  `FUN_30665500`: 2 sites, real ratio 0.72), not a VM.

## [I] Interpretation

* **Classification: (b)** - mutated/obfuscated x86 that a block follower can
  linearise. It is not (a): the decrypted code has rewritten prologues, SEH
  entry stubs, shuffled blocks, and push-ret continuations; the decompiler's
  "SEH-only stub" is the first block, not the function. It is not (c): the
  traversed blocks execute the original x86 operations on the original stack
  frame; there is no central bytecode fetch/decode loop or handler table.
* The decompiler's function boundaries and the `FUN_3a*` "stubs" are false
  negatives. The real code is reachable by treating the overwritten/pushed
  return addresses as successors.
* The unpacked file is a valid and deterministic static source for this build.
  The shipped packed file is not.
* The speed/jump/landing math is present in the decrypted image as native x86
  (`addss`/`mulss`/`fld`/`fstp`/vector copies), so a VM is not a barrier to
  porting it. The cost is in the flattening, not in missing code.

## [?] Open

* Full branch coverage of `FUN_3a979a34` and `FUN_3a98bb00`: the follower
  linearises a reachable subset (20 and 160 blocks; the latter hit the cap); it
  does not claim every branch, and indirect jumps through vtables remain
  unresolved.
* The semantics of the individual vector-math blocks are not decompiled here;
  the mapping from block to speed/jump/landing equation is W4/W5 work.
* The exact role of cryaction `0x307C70B0` and of `FUN_3a9918bb` is not
  established beyond their call position.
* Why the `.reloc` code cave is executable at run time (the file flag is not
  executable) was not queried; the evidence is that the code is present and
  called by the readable chain (W0).
* The official launcher/anti-cheat path was not tested; the lane harness has
  no anti-cheat module, and the probe refuses if one appears.

## What it means for porting speed/jump/landing

1. **Use `x2game_unpacked.dll` as the static source**, with the hashes above.
   Do not port from the shipped packed `x2game.dll` or from the decompiler's
   SEH-only stubs.
2. **Port the readable/linear pieces line by line first**:
   `FUN_3a6ab5f3` (the movement-state snapshot, 3 blocks / 91 instructions,
   0.86 real ops; it gives the actor/entity field layout), the callers
   `CActor::UpdateMovementRequest` (0x39118650), `ProcessFrameMovement`
   (0x39135FE0) and `ProcessFrameMovementRotation` (0x39135ED0), and the small
   `FUN_3a9918bb` thunk. The block follower recovers the split epilogues
   (`3913A753 -> 39123657` for `FUN_3a6ab5f3`).
3. **Measure behaviour for the large flattened functions**
   (`FUN_3a979a34`, `FUN_3a98bb00`) before attempting a full decompile. They
   contain the vector math and the branch structure, but they are spread over
   many blocks and the decompiler's boundaries are wrong. The W4 speed and W5
   fall/landing captures are the practical oracle; extract the specific
   equations from the live/linearised blocks when a behaviour difference
   points at them.
4. **Re-run the probe on any client build change.** The evidence checks 12/12;
   the live-equals-unpacked and cross-process checks are the gate. If a future
   build is not byte-identical across processes, stop using the static image
   and fall back to measured behaviour only.
