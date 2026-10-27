# Decompile validation: does the port rest on a correct reading of 1.2?

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Read-only audit of the addresses, globals, constants and map claims the ports use. No game code was
changed and no cargo build/test was run. Sources:

* Ghidra decompile of the unpacked 1.2 `x2game.dll` (`D:/aa-re/client12/out-x2game/x2game.dll.c`,
  `funcs.csv`, `calls.tsv`) and the 1.2 engine DLLs (`D:/aa-re/client12/engine*/out/`).
* An IDA 9.4 database of the same unpacked `x2game.dll` created read-only in
  `D:/aa-re/scratch/ida12-x2game/` (49,498 functions; x86 disassembly + ret imm16, no decompiler),
  plus the saved 1.2 IDA exports for `crysystem.dll` / `cryscriptsystem.dll`.
* The port sources (`crates/aa-client/src/port/**`) and the eight MAPs named in the task.
* `reference/` real-client captures.

Reproduce with `tools/decompile_validation/` (all paths are read-only):

```powershell
python -I tools/decompile_validation/collect_refs.py --root .
python -I tools/decompile_validation/parse_decompile.py --root .
python -I tools/decompile_validation/func_index.py --root .
python -I tools/decompile_validation/ida_disagreements.py --root .
python -I tools/decompile_validation/float10_report.py --root .
python -I tools/decompile_validation/globals_report.py --root .
python -I tools/decompile_validation/const_check.py --root .
```

Generated tables (git-ignored): `data/decompile_validation/*.tsv`.

## 1. Function addresses cited by the ports

### 1.1 Inventory

| Item | Count |
|---|---:|
| Address citations in `port/**` + the eight MAPs | 11,175 rows, 4,825 unique addresses |
| `FUN_` citations | 5,401 rows, **1,648 unique functions** |
| of which resolve to `x2game` | 1,252 |
| of which resolve to `crysystem` / `cryscriptsystem` | 29 / 1 |
| of which resolve to other engine DLLs (no IDA export) | 331 |
| ambiguous addresses present in both `x2game` and `crygame` | 18 |
| addresses present only in the IDA export | 9 |
| `FUN_` citations that are not a Ghidra function start (mid-function / data) | 8 |

The IDA `x2game` database has 49,498 functions; Ghidra has 65,437. IDA did not split the
obfuscated/exception tail, so the difference is expected, but it means “not in IDA” is not proof
that a cited function is wrong.

### 1.2 Start/end/size vs IDA

| Comparison | Count |
|---|---:|
| Cited `x2game` functions with an IDA match | 1,233 of 1,252 |
| exact start/end/size | **1,111 (90.1%)** |
| size differs | 122 |
| cited `crysystem` functions with an IDA match | 29 of 29 (27 exact) |
| cited `cryscriptsystem` functions | 1 of 1 (exact) |

Most `x2game` differences are IDA including 3–13 B of padding (32 × +3, 17 × +6, 9 × +9, …).
The large ones are real boundary disagreements and matter when a port copied “N bytes” as a scope
statement:

| Address | Ghidra | IDA | Δ | Port / MAP |
|---|---:|---:|---:|---|
| `393bc260` | 567 | 387 | −180 | `interaction/doodad.rs:23` (LOS/precheck helper) |
| `390e69d0` | 3,773 | 3,889 | +116 | `port-vehicles/MAP.md:395` |
| `3930f620` | 51 | 143 | +92 | `port-portals/MAP.md:78` |
| `393c17e0` | 699 | 790 | +91 | `interaction/builders.rs:276` |
| `393bd650` | 420 | 505 | +85 | `port-interaction/MAP.md:363` |
| `393b7440` | 876 | 945 | +69 | `interaction/functype.rs:8` |
| `39383f40` | 301 | 353 | +52 | `port-interaction/MAP.md:505` |
| `39202db0` | 463 | 504 | +41 | `combat/timeline.rs:31` |
| `390e96a0` | 4,268 | 4,268 | 0 | `anim/mount.rs:4` (see 1.3) |

Full list: `data/decompile_validation/disagreements.tsv` (kind `SIZE`).

### 1.3 The known `390e96a0` / 1,011 B disagreement

* `390e96a0` is **4,268 B** in Ghidra and **4,268 B** in IDA (`retn 8`, 2 stack args + `this`).
  Every MAP that sizes it says 4,268 (`port-mount-anim/B-mount-dismount.md:234,400`,
  `port-engine-anim/MAP.md:121,673`).
* The **1,011 B** function is its sibling `390dcc80` (rider copy), confirmed by both Ghidra and
  IDA (`port-mount-anim/E-onfoot-shared.md:31`, `C-riding-reactions.md:196`).
* Verdict: no MAP actually says `390e96a0` is 1,011 B; the disagreement is a mix-up between the
  two rider functions. This one is **not** a wrong Ghidra/IDA size.

### 1.4 Parameter count and calling convention

For `__thiscall`, `ret imm16 / 4 + 1` must equal Ghidra’s parameter count. 23 cited functions
fail that test; these are the real signature disagreements on the audited paths:

| Address | Ghidra params | IDA stack bytes | Declared | First port reference |
|---|---:|---:|---|---|
| `3910d230` | 3 | 12 | `__thiscall` | `port-engine-ai/MAP.md:209` |
| `3911b950` | 2 | 8 | `__thiscall` | `port-movement/MAP.md:155` |
| `3911f3f0` | 2 | 16 | `__thiscall` | `port-movement-extra/MAP.md:297` |
| `3911feb0` | 2 | 8 | `__thiscall` | `movement/glide.rs:283` |
| `3915d6f0` | 3 | 12 | `__thiscall` | `camera/mod.rs:8` |
| `3930f880` | 3 | 12 | `__thiscall` | `port-portals/MAP.md:71` |
| `3932e890` | 2 | 0 | `__thiscall` | `housing/fence.rs:10` |
| `39393920` | 3 | 0 | `__thiscall` | `port-combat/MAP.md:83` |
| `39395510` | 3 | 0 | `__thiscall` | `combat/cast.rs:6` |
| `3939c950` | 2 | 0 | `__thiscall` | `combat/timeline.rs:38` |
| `393a9540` | 4 | 0 | `__thiscall` | `housing/routing.rs:3` |
| `393b2230` | 4 | 16 | `__thiscall` | `interaction/bar.rs:8` |
| `393b3000` | 3 | 0 | `__thiscall` | `interaction/doodad.rs:38` |
| `393b7440` | 2 | 12 | `__thiscall` | `interaction/functype.rs:8` |
| `393bb090` | 3 | 0 | `__thiscall` | `interaction/functype.rs:26` |
| `393bcc20` | 2 | 0 | `__thiscall` | `interaction/candidates.rs:111` |
| `393bf7c0` | 2 | 8 | `__thiscall` | `interaction/npc.rs:181` |
| `39416c20` | 3 | 0 | `__thiscall` | `port-engine-core-rest/MAP.md:772` |
| `3941a700` | 4 | 0 | `__thiscall` | `port-formulas/MAP.md:66` |
| `394eaac0` | 3 | 0 | `__thiscall` | `housing/decor.rs:27` |
| `39508e10` | 2 | 0 | `__thiscall` | `port-vehicles/MAP.md:80` |
| `39567740` | 2 | 0 | `float10 __thiscall` | `formulas/eval.rs:10` |
| `398b0150` | 3 | 0 | `__thiscall` | `formulas/mod.rs:12` |

Two cases are worth singling out:

* `39209d80` (action-slot native dispatch, `combat/bar.rs:13`): Ghidra prints **one** parameter,
  the decompile reads `in_stack_00000028/2c/30`, and IDA shows `retn 48` at three returns
  (12 stack dwords → 13 with `this`). The port’s one-argument signature is incomplete.
* `39393920` (auto-attack start, `port-combat/MAP.md:83`): Ghidra says `__thiscall(3 params)`,
  IDA finds only `retn` (no stack cleanup) at both returns. The convention is unresolved; the
  port should re-prove which of `caster`/`target` are stack vs register args.

Systematic caveat: 267 cited `__fastcall` functions have Ghidra naming only one parameter
(`EDX` is `unaff_EDX`); that is a decompiler presentation issue, not 267 binary disagreements.
127 more have no convention in the decompile. Full list: `disagreements.tsv` (kind `PARAMS`).

### 1.5 Cited functions missing from IDA

19 `x2game` citations have no IDA function at the address. The obfuscated tail accounts for the
`3a…` ones; the rest are IDA analysis gaps, not necessarily wrong ports:

| Address | Ghidra size | First port reference |
|---|---:|---|
| `3909c660` | 1,837 | `vehicles/controller.rs:114` |
| `390a3de0` | 1,175 | `vehicles/controller.rs:7` |
| `390ac300` | 29 | `vehicles/step.rs:3` |
| `390acee0` | 85 | `vehicles/controller.rs:6` |
| `39154060` | 42 | `port-engine-combat/MAP.md:874` |
| `39186db7` | 175 | `port-combat/MAP.md:151` |
| `39331760` | 123 | `housing/lifecycle.rs:9` |
| `39390ec0` / `39390f20` | 81 / 95 | `combat/targeting.rs:19` |
| `393a0510` | 1,087 | `combat/requirement.rs:3` |
| `393a9340` | 392 | `housing/seeds.rs:16` |
| `3ab63fb7` | 114 | `combat/front.rs:3` |
| `3ac1058c` | 24 | `port-combat/MAP.md:259` |

`393a0510` is the largest gap and the one that matters most: the 47-evaluator unit-requirement
walker is a ported, tested function that IDA did not turn into code at all. The port can only be
checked against Ghidra there.

### 1.6 Ambiguous module attribution

These 18 cited addresses exist at the **same VA in both `x2game` and `crygame`** with different
bodies. A comment that says “x2game” is not enough; the port must pin the module. Example:
`39093860` is 337 B in `x2game` (the `sqrt(radius)` range helper the interaction port uses) and
90 B in `crygame`.

`3902d670`, `39093860`, `39096a10`, `3909ae90`, `390a8dc0`, `390d14b0`, `390f4c30`, `391136c0`,
`3912bfb0`, `3912dd40`, `3913dfd0`, `39156310`, `3915b0a0`, `3916bc70`, `391b90a0`, `391da940`,
`391dde80`, `391e55c0`.

### 1.7 The survey’s `gEnv+0x54` vs x2game’s `*(gEnv+0x2C)`

Confirmed. `DAT_3a5bb1d8` is `gEnv`; the x2game decompile has **270** sites of
`*(int **)(DAT_3a5bb1d8 + 0x2c)` (the 3D-engine interface used for terrain/water queries) and 19
sites of `+0x54`, which are a different interface. `CSystem+0x54 == gEnv+0x2C`, so the survey
mixed the `CSystem` and `gEnv` bases. `port-engine-survey/audit-world.md:35-53` already has the
correction; this audit confirms it from the decompile counts.

## 2. float10 (x87 80-bit) paths

`float10` appears in 3,998 x2game decompile lines. Of the 1,252 cited x2game functions:

| Item | Count |
|---|---:|
| cited functions whose decompile contains `float10` | **214** |
| of those with a decompile body | 203 |
| of those cited by actual port code (not only a MAP) | **127** |
| cited functions with no decompile body | 11 |

Appendix C lists all 121 port-referenced addresses that have a decompile body. Top paths with
real x87 arithmetic:

| Address | float10 refs | Port module | Test style |
|---|---:|---|---|
| `39736bd0` | 32 | `vehicles/model.rs` | EXACT (`assert_eq` only) |
| `39161690` | 29 | `camera/mod.rs`, `mode1.rs` | EXACT (6 exact, 0 approx) |
| `39016fe0` | 25 | `housing/builder.rs` | EXACT (102 exact, 0 approx) |
| `39404d50` | 22 | `movement/input_state.rs`, `request.rs` | EXACT (`assert_bits`) |
| `39162d00` | 18 | `camera/basis.rs` | EXACT |
| `3913f020` / `3913f710` | 14 / 14 | `movement/rotation.rs` | EXACT (`assert_bits`) |
| `39402e00` | 14 | `movement/turn.rs` | EXACT (`assert_bits`; x87 rate emulated with f64) |
| `390a3de0` | 13 | `vehicles/controller.rs` | EXACT |
| `390aaf30` | 13 | `anim/mount.rs` | mixed (anim: 146 exact / 10 approx) |
| `3915c700` / `3915c7d0` | 11 / 11 | `movement/mouse.rs` | EXACT (`assert_bits`; f64 emulation) |
| `39165420` | 11 | `camera/collision.rs` | EXACT |
| `39426d10` | 12 | `combat/front.rs` | EXACT (179 exact / 1 approx) |

Verdict: the movement/camera/vehicles paths that do x87 math are tested **bit-exactly**, not with a
tolerance. That is a strong constraint, but it also means every exact golden is only as good as the
Ghidra reading of the x87 sequence. The port explicitly emulates 80-bit only in `movement/mouse.rs`
and `movement/turn.rs`; the other 100+ float10 functions rely on f32/f64 Rust math and whatever the
golden captured. `anim`/`cryanimation` are the only modules with meaningful tolerance use
(10 and 6 approximate assertions). Re-proof priority: any float10 function whose test is EXACT but
whose Rust code does not call out the extended-precision step.

## 3. DAT_ globals and vtable slots

`globals.tsv` / `vtables.tsv` list every DAT_/slot cited by port code.

| Class | DAT_ | vtable slots |
|---|---:|---:|
| `STATIC` (bytes readable in `.rdata` or initialized `.data`) | 100 | 57 |
| `RUNTIME` (`.data` zero at rest: BSS/heap pointer or runtime-written global) | 88 | 0 |
| `LIVE` (port comment cites a live capture/golden) | 3 | 3 |
| `ASSUMED` (explicit inference) | 0 | 0 |

The 88 runtime ones are mostly anchors (e.g. `DAT_3a5bb1d8` = gEnv, `DAT_39b02294` = game
client, `DAT_39aff9bc` = interaction manager). Using their **address** is safe; baking their
**contents** is not. The ports correctly take most cvar/config values as inputs.

The classifier flags 3 DATs as `LIVE`; only two are genuine live evidence, one is a debug-only read:

| Address | Evidence |
|---|---|
| `DAT_39ae4e90` | `vehicles/apply.rs:107`: live capture records `->vt[8]` per case |
| `DAT_3a177700` | `combat/front.rs:346,356`: live object id; empty sentinel `0xfffffffe` |
| `DAT_316c9248` | `cryanimation/process.rs:168`: optional debug FIFO dump, no state effect |

The 3 slots flagged `LIVE` (`0xc`, `0x58`, `0x1dc`) are decompile call-site offsets; the flag comes
from “live/capture” wording in the comments, not from a value read out of a runtime object. They
are static offsets and are safe as offsets.

### 3.1 Constants whose documented DAT does not hold the value

`const_check.py` paired 121 numeric constants with the DAT in the doc comment. 11 disagree with
the shipped static bytes. Most are **citation** errors, not wrong values: the constant is an
immediate in the function or is written at runtime.

| Port constant | Cited DAT (static value) | Port value | Verdict |
|---|---|---|---|
| `DEFAULT_ACTION` (`combat_anim/scheduler.rs:28`) | `39ac3f20` = **1** | 0 | **real mismatch**: `FUN_391555d0` uses `DAT_39ac3f20` as the default; the shipped value is 1, the port bakes 0 |
| `WEAPON_KEEP` (`combat_anim/entry.rs:38`) | `39afd374` = 0 | 3 | value **confirmed** by `FUN_39157680:50` (`param_4 == 3`); the DAT citation is wrong |
| `MY_TEAM` (`combat/targeting.rs:31`) | `39d44c50` = 0 (local-player pointer) | 1 | value is the immediate/flag set in `FUN_394bd9f0`; DAT citation is wrong |
| `NO_NEAREST_HOUSE` (`housing/routing.rs:38`) | `3999cffc` = 2.0 (search radius) | 199 | 199 is an immediate in `FUN_394eaac0:100`; DAT citation is wrong |
| `PLOT_HALF_MAX` (`housing/fence.rs:30`) | `3999c488` = 1.0 | 12 | 12 is a comparison immediate in `FUN_39897170`; DAT citation is wrong |
| `CURSOR_COOLDOWN_MS` (`housing/decor.rs:61`) | `39adcc24` = 0 | 1000 | code writes `now + 1000` (`x2game.dll.c:58608`); DAT citation is wrong |
| `DISMISS_COOLDOWN_MS` (`anim/summon.rs:50`) | `3a169944` = 0 | 2000 | code tests `1999 < diff`; derived threshold, DAT citation is wrong |
| `MATE_UNIT_SENTINEL` (`anim/summon.rs:48`) | `3a1682f4` = 0 | `0xffff_ffff` | runtime-initialized sentinel; not statically proven |
| `CATEGORY_EMPTY` (`combat/front.rs:357`) | `3a177700` = 0 | `0xffff_fffe` | runtime-initialized sentinel; not statically proven |
| `CREATE_RATE_LIMIT_MS` (`housing/lifecycle.rs:216`) | `3a167ea0` = 0 | 5000 | runtime-written global; not statically proven |
| `SEED_GUARD_MS` / `SEED_CURSOR_MODE` (`housing/seeds.rs:55,57`) | `3a17a950` / `3a5a4ea0` = 0 | 2000 / 7 | runtime-written globals; not statically proven |

The last four are the “assumed” findings the task asked for: a live probe or a golden must back
them before they are treated as proven.

## 4. Sample verification of MAP [R] claims

40 claims across the eight MAPs; line numbers are in the named file.

| # | MAP | Claim | Verdict | Evidence |
|---:|---|---|---|---|
| 1 | `port-movement/MAP.md:319` | `FUN_394037d0`: 1 normal, 5 scripted, 6 cinematic, 0 not allowed | CONFIRMED | returns 5 on `+0x3a78`, 6 when `+0x3b00 != DAT_3a59f864`, 0 via `FUN_39354600`, else 1 |
| 2 | `port-movement/MAP.md:378` | on-foot turn = `axis * min(dt,0.033) * 0.5 * π` = 90°/s | CONFIRMED | `FUN_39402e00` returns `axis*dt*cvar*π`; `0x3999c47c` = 0.5, `0x399a3040` = 0.033 |
| 3 | `port-movement/MAP.md:424` | `FUN_3915c700` is 193 B and reads axis 8 | CONFIRMED | `funcs.csv` 193 B; call `FUN_39424580(8)` |
| 4 | `port-movement/MAP.md:211` | action table `0x39ac8e68`, index 0 = `moveforward` | CONFIRMED | `.data` pointer → string `moveforward` at `0x399d5d04` |
| 5 | `port-movement/MAP.md:467` | camera update uses `min(dt,0.033)` (`DAT_399a3040`) | CONFIRMED | `FUN_3916aab0` compares against `DAT_399a3040`; bytes = 0.033 |
| 6 | `port-mount-anim/E-onfoot-shared.md:31` | rider copy `390dcc80` is 1,011 B | CONFIRMED | Ghidra 1,011, IDA 1,011 |
| 7 | `port-mount-anim/B-mount-dismount.md:234` | seat bind `390e96a0` is 4,268 B | CONFIRMED | Ghidra 4,268, IDA 4,268, `retn 8` |
| 8 | `port-mount-anim/MAP.md:92` | attach point is `param_2[0x4b]` (1/2) | CONFIRMED | `FUN_390e96a0` line 87 reads `param_2[0x4b]` |
| 9 | `port-engine-loop/MAP.md:52` | `X2Startup` vt+0x18 = `Run` `3950c4f0` | CONFIRMED | vtable `399e6a38+0x18` → `3950c4f0`, 452 B |
| 10 | `port-engine-loop/MAP.md:74` | `CGame::Update` profiler string at `399a6d1c` | CONFIRMED | bytes `43 47 61 6d 65 3a 3a 55` = “CGame::U…” |
| 11 | `port-engine-loop/MAP.md:105` | `FUN_306f66e0` (add listener) is 738 B | CONFIRMED | cryaction `funcs.csv` 738 |
| 12 | `port-engine-loop/MAP.md:109` | `CCryAction` vtable `3074fa60`, +0x114 = `306f66e0` | CONFIRMED | `.rdata` pointer at `3074fb74` = `306f66e0` |
| 13 | `port-engine-loop/MAP.md:210` | `IEntitySystem::PrePhysicsUpdate` = `3201c890`, 506 B | CONFIRMED | cryentitysystem `funcs.csv` 506 |
| 14 | `port-engine-loop/MAP.md:253` | `0x311b6484` = 0.1 s | CONFIRMED | cryaisystem `.rdata` f32 = 0.1 |
| 15 | `port-engine-loop/MAP.md:296` | `Client Game Update` called from `0x390f6c5f` | CONFIRMED | bytes at `390f6c5f` = `E8 7C CD 07 00` → `391739e0` |
| 16 | `port-engine-loop/MAP.md:338` | timer vtable `3660c060`, +0x30 = `3656e6d0` | CONFIRMED | `.rdata` pointer at `3660c090` = `3656e6d0` |
| 17 | `port-engine-loop/MAP.md:143` | `CSystem::Render` `36561650` is 1,389 B | PARTIAL | Ghidra 1,389, IDA 1,397 (8 B padding/boundary) |
| 18 | `port-engine-char/MAP.md:120` | `ProcessEvent` caches `ICharacterInstance` at `AC+0x738` | **WRONG** | `FUN_30663ac0` contains no `0x738`; writes are in `30661a20`, `306629f0`, `306634a0` |
| 19 | `port-engine-char/MAP.md:133` | `FUN_390287a0` 361 B sets `actor+0x464` and calls `vt+0x1e0(AC vt+0x50)` | CONFIRMED | crygame `funcs.csv` 361; decompile lines 28–32 |
| 20 | `port-engine-char/MAP.md:188` | `Unit::AttachTo` `390b3f10` 225 B → `vt+0x278/+0x288` via `390e16b0` 94 B | CONFIRMED | both sizes exact; decompile shows both slots |
| 21 | `port-engine-char/MAP.md:400` | constants 2.0/0.001/57.29578/1/60/0.999 | CONFIRMED | all six `.rdata` f32 values match |
| 22 | `port-engine-anim/MAP.md:109` | `FUN_390e96a0` uses `vt[0x168]/[0x20]/[0x1c]/[0x34]/[0x94]/[0x48]/[0x3c]` | CONFIRMED | all seven call sites in the body |
| 23 | `port-engine-anim/MAP.md:121` | seat attach `390e96a0` is 4,268 B | CONFIRMED | Ghidra + IDA 4,268 |
| 24 | `port-engine-combat/MAP.md:383,964` | `FUN_391547b0` is 72 B | **WRONG** | Ghidra **60**, IDA **60**, `retn 4` |
| 25 | `port-engine-combat/MAP.md:371-372` | `39156760` 394 B, `39156900` 410 B | PARTIAL | Ghidra 394/410 exact; IDA 394/**433** for `39156900` |
| 26 | `port-combat/MAP.md:90` | `FUN_3939a440` called only from `3939a810` | CONFIRMED | `calls.tsv` has exactly one caller |
| 27 | `port-combat/MAP.md:131` | cooldown manager at `unit+0x2c48`, `lea ecx` at `3939aab9`/`3939aad7` | CONFIRMED | bytes `8D 8E 48 2C 00 00` at both |
| 28 | `port-combat/MAP.md:174` | action indices 22/23/39/40/41/42/82/84/85/86/87/89 | CONFIRMED | table pointers decode to the named strings |
| 29 | `port-combat/MAP.md:188` | `FUN_392c4dd0` is 725 B; fires event 0x25 | PARTIAL | size **WRONG** (485 B); event `0x25` confirmed at two call sites |
| 30 | `port-combat/MAP.md:195` | 85 action slots, scan `iVar3 = 0x55` | CONFIRMED | `FUN_39089c80` lines 15–25 |
| 31 | `port-combat/MAP.md:201` | `0x3943f420` is a cooldown query | CONFIRMED | function 78 B at that start |
| 32 | `port-combat/MAP.md:228` | `UseSkill` (`393989e0`, 657 B) step order and thunk | CONFIRMED | 657 B; calls appear in the stated order, thunk `3ab63fb7` at line 104 |
| 33 | `port-interaction/MAP.md:71` | `do_interaction_1..4` = 31..34, `quick_interaction` = 72 | CONFIRMED | table pointers decode to those strings |
| 34 | `port-interaction/MAP.md:81` | `FUN_393b2b70` is a 20 B wrapper; four stubs are absent from `funcs.csv` | CONFIRMED | 20 B; `393f8290..c0` not function starts |
| 35 | `port-interaction/MAP.md:118` | `UpdatePicking` `3942c640` calls `394273e0`, `39428d80`, `39354210`, `39358c00`, `393b51a0` | CONFIRMED | all five calls present; size 976 B |
| 36 | `port-interaction/MAP.md:134` | ray masks `0x8002048f`/`0x117`, then `0x200048f`/`0x11f`; pick string layout | CONFIRMED | `FUN_39428d80` lines 76/83; string at `399d87a0` has matching format |
| 37 | `port-interaction/MAP.md:144` | `390f4960` is normal click: mounted return, `39429e40`, `393be4e0`, `390f3780` | CONFIRMED | 715 B; all calls present, mount flag at `gEnv+0x97` |
| 38 | `port-interaction/MAP.md:154` | `FUN_390f3780` dispatches unit/doodad/gimmick | CONFIRMED | calls `393be030`/`393b79a0`/`39429d00` |
| 39 | `port-interaction/MAP.md:163` | `FUN_393b79a0` calls `FUN_393b3ec0(skill, 0, doodad, 1)` | CONFIRMED | decompile passes `DAT_3a17d7d8`, static value 0 |
| 40 | `port-interaction/MAP.md:145` | cancel check compares skill to 10994 (`DAT_3a17d7d0`) **and** 10994 (`DAT_39ac7820`) | **WRONG** | `DAT_3a17d7d0` = 0 (“none”); only `DAT_39ac7820` = 10994 |
| 41 | `port-interaction/MAP.md:417` | 3.2 packet is `{0x6B, targetId, id, var1}` | **WRONG** | `FUN_393b3120` stores `0x6B` in a separate header field; `FUN_397ac0d0` stores **3 caller-supplied dwords** at `+0xc/+0x10/+0x14`; the caller passes two ids, so `var1` is not a stored field of this call |
| 42 | `port-interaction/MAP.md:208` | `FUN_393be280` order: skill ids, mounted 0x14c, rate limit, target gate, 3 m gate, select, services, quest, registry, CSInteractNPC, face | CONFIRMED | decompile lines 21–84 in that order |
| 43 | `port-interaction/MAP.md:340` | bar LOS `FUN_393bc980` uses camera pos and ray flags `0x2048f` | CONFIRMED | body builds camera pos, flag `0x2048f`, returns `<1` |
| 44 | `port-interaction/MAP.md:170-173` | SelectInteraction path `3947dac0 → 393b3120`, fallback `393b3ec0` | CONFIRMED | decompile shows both branches |
| 45 | `port-engine-loop/MAP.md:155` | `CSystem::Update` is 4,469 B | CONFIRMED | Ghidra 4,469 |

Totals: **38 confirmed, 4 wrong, 3 partial**. The known wrong items requested by the task are all
present: 3.2 packet layout (#41),
cancel-id constant (#40), `+0x738` ProcessEvent attribution (#18), the rider-size mix-up (§1.3)
and the `gEnv+0x54` / `*(gEnv+0x2C)` survey error (§1.7). The “E3 vs program-note ±45° source”
disagreement is recorded in §4.1.

### 4.1 E3 vs program-note: where the rider’s ±45° comes from

* `docs/research/port-engine-anim/STEP-D.md:57-60` derives the lean from the rider’s **movement
  direction** (`Motion.local_dir`, `atan2(-x,y)`, clamp ±90°) and calls it a measured rule.
* `docs/research/port-program.md:65` says the **request-direction version from
  `port/engine-char-a` takes precedence** when the two meet on the riding-build chain.
* The shipped `ingame/mount_anim.rs` uses the measured `rider_lean` rule; `FUN_390dcc80` itself
  copies only PseudoSpeed/Stance/TurnSpeed and does **not** set the rider yaw (confirmed from its
  1,011 B body). So the decompile does not choose between the two rules: both are layer-2
  reconstructions. This is a documentation disagreement, not a decompile-verifiable fact, and it
  should be resolved by a live A/B test before either note is called proven.

## 5. Reference captures

`reference/README.md` is stale: it indexes the UI screens but not the `ingame/combat_*` and
`ingame/mount_*` folders that exist on disk. Actual inventory:

| Slice | Files | What it would check | Used by the port/tests/docs? |
|---|---|---|---|
| `ingame/spawn` | 4 (`real.png`, `real_first_frame.png`, `ui.json`, `camera.json`) | first-frame HUD, spawn camera pose/FOV, entity position | **Yes**, heavily (`aa-ui` examples, HUD, camera, marks; 52 refs) |
| `ingame/combat_melee` | 19 frames, notes, frames.json | sword draw, swing timing, first damage ~0.85 s, HUD target/HP | Docs/visual only (`aa-client-combat.md`, `aa-ui.md`, `aa-anim/character.rs`); no port test/golden |
| `ingame/combat_triple_slash` | 18 frames, notes, frames.json | skill 18132, three cuts at ~0.85/1.4/2.5 s, 26/26/31 damage, weapon trail | Docs/visual only (HUD/water/anim comments); no port test/golden |
| `ingame/combat_kick` | 11 frames, notes | skill 14495, ~1 m lunge, one hit 12 | Docs only (`aa-client-combat.md`); no test/golden |
| `ingame/combat_return` | 25 frames, notes | 10 s Recall cast bar, orb radius ~0.39 m, zone reload | Docs/visual only (`portals.md`, `aa-fx.md`, HUD/usable) |
| `ingame/combat_death`, `_2` | 20 + 21 frames, notes | death throw/corpse, HUD darkening, shield-up-after-kill | Docs/visual + a few HUD comments; no test/golden |
| `ingame/mount_mount` | 71 frames in five sides (`front/back/left/right/frontside`), frames_click_*.json | seat/rider orientation from five camera sides, mount pose | **No references anywhere in the repo** |
| `ingame/mount_dismount` | 55 frames | dismount sequence, seat release, rider re-pose | **No references anywhere in the repo** |
| `ingame/mount_summon` | 32 frames | summon spawn/seat attach | **No references anywhere in the repo** |

The `combat_*` timing claims (0.85/1.4/2.5 s, first hit 0.85 s) are written in docs but never
turned into a test. The five `mount_mount` sides are the natural oracle for the ±45° seat/lean
question and the seat offset; they are unused. That is the biggest capture gap in this audit.

## 6. Top risks and what to re-prove first

1. **`39209d80` action-slot dispatch** (`combat/bar.rs`): Ghidra signature is incomplete
   (1 param vs 12 stack dwords + `this`). Re-prove the argument order before trusting it.
2. **`393a0510` 47-evaluator unit-requirement walker** (`combat/requirement.rs`): the only large
   ported function with no IDA function at all (1,087 B in Ghidra). Re-run IDA analysis or
   hand-disassemble; every requirement test rests on one decompile.
3. **`391547b0` weight setter**: documented as 72 B in two MAPs and the port; the binary is 60 B.
   The “72 B” scope statement is wrong even if the semantics are right.
4. **`392c4dd0` action-bar button handler**: documented 725 B, actual 485 B. The event-0x25
   behavior is confirmed, the size/scope is not.
5. **`393bc260` interaction precheck**: Ghidra 567 B vs IDA 387 B (−180). Decide which body is
   the function before relying on the ported branch order.
6. **`39156e50` record copy**: Ghidra 4 params, IDA `retn` with no stack cleanup. Calling
   convention (cdecl vs thiscall) is unresolved; the port’s `AnimEventRecord` layout depends on it.
7. **`39393920` auto-attack start** and **`393b3000`/`393b2230`/`393b7440` interaction helpers**:
   Ghidra `__thiscall` parameter counts disagree with IDA ret imm16; re-prove which args are stack
   vs register.
8. **`390e96a0` seat bind**: size is right (4,268), but it is the biggest cited function (1,252
   x2game citations include it) and its 1,011 B sibling is easy to confuse with it. Keep the two
   separate in every future note.
9. **`DEFAULT_ACTION = 0`** (`combat_anim/scheduler.rs:28`): the shipped `DAT_39ac3f20` is 1.
   Either change the constant or prove a runtime write.
10. **The four assumed runtime sentinels** (`MATE_UNIT_SENTINEL`, `CATEGORY_EMPTY`,
    `CREATE_RATE_LIMIT_MS`, `SEED_GUARD_MS`/`SEED_CURSOR_MODE`): static bytes are zero; they need
    a live probe or golden before being called `[R]`.
11. **x87 exactness on the remaining 100+ float10 functions**: EXACT tests are only meaningful if
    the Rust code reproduces the 80-bit intermediate. Only mouse/turn explicitly do so. Start with
    `39404d50` (movement request), `39161690` (camera mode 1), `39736bd0` (vehicle model) and
    `39426d10` (combat front), which are large and EXACT-tested.
12. **`393b3120` packet layout**: fix the MAP (3 stored dwords, caller-supplied ids, `0x6B` is a
    separate header field) before any packet-golden work.

Other doc-level corrections to make in place: `port-interaction/MAP.md:145` (cancel ids),
`port-engine-char/MAP.md:120` (ProcessEvent), `port-engine-combat/MAP.md:383,964` (72 B),
`port-engine-combat/MAP.md:188` (725 B), and the `DAT_` citations in §3.1.

## Appendix A. Every DAT_ cited by port code (191)

| DAT | class | section / bytes | first port reference |
|---|---|---|---|
| `307435ec` | STATIC | `cryaction=2e616700(f32 9.49391761e-39)` | `crates\aa-client\src\port\anim\ag.rs:5` |
| `31674584` | STATIC | `cryanimation=0000003f(f32 0.5)` | `crates\aa-client\src\port\anim\lmg.rs:542` |
| `316745a0` | STATIC | `cryanimation=0000803f(f32 1)` | `crates\aa-client\src\port\anim\lmg.rs:16` |
| `316745a4` | STATIC | `cryanimation=00000040(f32 2)` | `crates\aa-client\src\port\cryanim\mod.rs:83` |
| `316745e0` | STATIC | `cryanimation=ffffff7f(f32 nan)` | `crates\aa-client\src\port\anim\lmg.rs:18` |
| `316745f0` | STATIC | `cryanimation=cdcccc3d(f32 0.100000001)` | `crates\aa-client\src\port\anim\lmg.rs:20` |
| `316746f4` | STATIC | `cryanimation=0ad7233c(f32 0.00999999978)` | `crates\aa-client\src\port\cryanim\mod.rs:89` |
| `31675118` | STATIC | `cryanimation=acc52737(f32 9.99999975e-06)` | `crates\aa-client\src\port\anim\lmg.rs:537` |
| `31676590` | STATIC | `cryanimation=cdcc4c3f(f32 0.800000012)` | `crates\aa-client\src\port\anim\lmg.rs:535` |
| `31676634` | STATIC | `cryanimation=9a99193f(f32 0.600000024)` | `crates\aa-client\src\port\anim\lmg.rs:539` |
| `316768ac` | STATIC | `cryanimation=6666663f(f32 0.899999976)` | `crates\aa-client\src\port\cryanim\mod.rs:75` |
| `31676938` | STATIC | `cryanimation=17b7d138(f32 9.99999975e-05)` | `crates\aa-client\src\port\cryanim\mod.rs:93` |
| `31676a00` | STATIC | `cryanimation=cdcc4c3e(f32 0.200000003)` | `crates\aa-client\src\port\anim\lmg.rs:26` |
| `31677bf0` | STATIC | `cryanimation=b9fcff3f(f32 1.99989998)` | `crates\aa-client\src\port\cryanim\mod.rs:73` |
| `31677bf4` | STATIC | `cryanimation=666666bf(f32 -0.899999976)` | `crates\aa-client\src\port\cryanim\mod.rs:77` |
| `31677bf8` | STATIC | `cryanimation=9a99d93f(f32 1.70000005)` | `crates\aa-client\src\port\cryanim\mod.rs:79` |
| `3167818c` | STATIC | `cryanimation=0000003e(f32 0.125)` | `crates\aa-client\src\port\anim\lmg.rs:24` |
| `31678500` | STATIC | `cryanimation=cdcc0c3f(f32 0.550000012)` | `crates\aa-client\src\port\anim\lmg.rs:539` |
| `316c2254` | RUNTIME | `cryanimation=00000000(f32 0)` | `crates\aa-client\src\port\cryanim\process.rs:190` |
| `316c9248` | LIVE | `-` | `crates\aa-client\src\port\cryanim\process.rs:168` |
| `316c948c` | RUNTIME | `-` | `crates\aa-client\src\port\anim\command.rs:43` |
| `316c94bc` | RUNTIME | `-` | `crates\aa-client\src\port\cryanim\process.rs:184` |
| `3999c478` | STATIC | `x2game=0000c03f(f32 1.5)` | `crates\aa-client\src\port\camera\state.rs:230` |
| `3999c47c` | STATIC | `x2game=0000003f(f32 0.5)` | `crates\aa-client\src\port\movement\quadruped.rs:22` |
| `3999c480` | STATIC | `x2game=00000034(f32 1.1920929e-07)` | `crates\aa-client\src\port\camera\state.rs:232` |
| `3999c484` | STATIC | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\reaction.rs:14` |
| `3999c488` | STATIC | `x2game=0000803f(f32 1)` | `crates\aa-client\src\port\movement\quadruped.rs:20` |
| `3999cc10` | STATIC | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:245` |
| `3999cc18` | STATIC | `x2game=00008000(f32 1.17549435e-38)` | `crates\aa-client\src\port\camera\state.rs:243` |
| `3999cc1c` | STATIC | `x2game=000080bf(f32 -1)` | `crates\aa-client\src\port\combat_anim\entry.rs:44` |
| `3999cce8` | STATIC | `x2game=00002041(f32 10)` | `crates\aa-client\src\port\combat_anim\entry.rs:54` |
| `3999ccf0` | STATIC | `x2game=cdcccc3d(f32 0.100000001)` | `crates\aa-client\src\port\combat_anim\events.rs:5` |
| `3999cf30` | STATIC | `x2game=6f12833a(f32 0.00100000005)` | `crates\aa-client\src\port\movement\glide.rs:9` |
| `3999cf90` | STATIC | `x2game=0ad7233c(f32 0.00999999978)` | `crates\aa-client\src\port\camera\collision.rs:203` |
| `3999cf98` | STATIC | `x2game=35fa8e3c(f32 0.0174532924)` | `crates\aa-client\src\port\camera\state.rs:257` |
| `3999cffc` | STATIC | `x2game=00000040(f32 2)` | `crates\aa-client\src\port\housing\fence.rs:21` |
| `3999d500` | STATIC | `x2game=ffffff7f(f32 nan)` | `crates\aa-client\src\port\housing\collide.rs:16` |
| `3999d608` | STATIC | `x2game=ffff7f7f(f32 3.40282347e+38)` | `crates\aa-client\src\port\interaction\range.rs:37` |
| `3999dd34` | STATIC | `x2game=cdcc4c3e(f32 0.200000003)` | `crates\aa-client\src\port\movement\breath.rs:103` |
| `3999dda8` | STATIC | `x2game=000000bf(f32 -0.5)` | `crates\aa-client\src\port\camera\state.rs:228` |
| `3999ddc8` | STATIC | `x2game=00004842(f32 50)` | `crates\aa-client\src\port\housing\ray.rs:18` |
| `3999f7c0` | STATIC | `x2game=0000f041(f32 30)` | `crates\aa-client\src\port\movement\quadruped.rs:34` |
| `399a1d94` | STATIC | `x2game=cdcc4c3f(f32 0.800000012)` | `crates\aa-client\src\port\housing\fence.rs:19` |
| `399a1ebc` | STATIC | `x2game=17b7d138(f32 9.99999975e-05)` | `crates\aa-client\src\port\camera\state.rs:234` |
| `399a232c` | STATIC | `x2game=cdcc4c3d(f32 0.0500000007)` | `crates\aa-client\src\port\combat_anim\entry.rs:48` |
| `399a2510` | STATIC | `x2game=7f9618cb(f32 -9999999)` | `crates\aa-client\src\port\camera\state.rs:240` |
| `399a2514` | STATIC | `x2game=7f96184b(f32 9999999)` | `crates\aa-client\src\port\camera\state.rs:240` |
| `399a27c0` | STATIC | `x2game=db0fc93f(f32 1.57079637)` | `crates\aa-client\src\port\combat_anim\entry.rs:56` |
| `399a2994` | STATIC | `x2game=d7a3703e(f32 0.234999999)` | `crates\aa-client\src\port\movement\quadruped.rs:31` |
| `399a2998` | STATIC | `x2game=8fc2f53e(f32 0.479999989)` | `crates\aa-client\src\port\movement\quadruped.rs:30` |
| `399a2ea0` | STATIC | `x2game=f304b53e(f32 0.353553385)` | `crates\aa-client\src\port\camera\state.rs:249` |
| `399a2ea8` | STATIC | `x2game=f168e388(f32 -1.36867371e-33)` | `crates\aa-client\src\port\camera\state.rs:247` |
| `399a2eb0` | STATIC | `x2game=f304353f(f32 0.707106769)` | `crates\aa-client\src\port\camera\state.rs:251` |
| `399a2f24` | STATIC | `x2game=cdccccbd(f32 -0.100000001)` | `crates\aa-client\src\port\movement\quadruped.rs:25` |
| `399a319c` | STATIC | `x2game=00004843(f32 200)` | `crates\aa-client\src\port\housing\ray.rs:17` |
| `399a321c` | STATIC | `x2game=0bd7233c(f32 0.0100000007)` | `crates\aa-client\src\port\movement\swim.rs:124` |
| `399a3ab0` | STATIC | `x2game=00401c46(f32 10000)` | `crates\aa-client\src\port\camera\state.rs:238` |
| `399a44f4` | STATIC | `x2game=e02e6542(f32 57.2957764)` | `crates\aa-client\src\port\movement\quadruped.rs:33` |
| `399aa82c` | STATIC | `x2game=fee6db2e(f32 9.99999944e-11)` | `crates\aa-client\src\port\combat_anim\projectile.rs:54` |
| `399ac39c` | STATIC | `x2game=0bd7233b(f32 0.00250000018)` | `crates\aa-client\src\port\movement\swim.rs:124` |
| `399acc40` | STATIC | `x2game=0e74da3b(f32 0.00666666683)` | `crates\aa-client\src\port\movement\quadruped.rs:35` |
| `399c66a8` | STATIC | `x2game=0000fa43(f32 500)` | `crates\aa-client\src\port\housing\ray.rs:18` |
| `399d2450` | STATIC | `x2game=25000000(f32 5.18480432e-44)` | `crates\aa-client\src\port\housing\seeds.rs:113` |
| `399d2458` | STATIC | `x2game=25000000(f32 5.18480432e-44)` | `crates\aa-client\src\port\housing\seeds.rs:113` |
| `39ac3044` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat_anim\events.rs:18` |
| `39ac3f00` | STATIC | `x2game=0e000000(f32 1.96181785e-44)` | `crates\aa-client\src\port\combat_anim\entry.rs:24` |
| `39ac3f04` | STATIC | `x2game=11000000(f32 2.38220739e-44)` | `crates\aa-client\src\port\combat_anim\entry.rs:24` |
| `39ac3f08` | STATIC | `x2game=ff000000(f32 3.57331108e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:21` |
| `39ac3f0c` | STATIC | `x2game=00010000(f32 3.58732407e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:21` |
| `39ac3f10` | STATIC | `x2game=82010000(f32 5.40901207e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:27` |
| `39ac3f14` | STATIC | `x2game=83010000(f32 5.42302506e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:27` |
| `39ac3f18` | STATIC | `x2game=a1010000(f32 5.8434146e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:30` |
| `39ac3f1c` | STATIC | `x2game=a2010000(f32 5.85742758e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:30` |
| `39ac3f20` | STATIC | `x2game=01000000(f32 1.40129846e-45)` | `crates\aa-client\src\port\combat_anim\scheduler.rs:27` |
| `39ac3f28` | STATIC | `x2game=03010000(f32 3.62936302e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:35` |
| `39ac3f38` | STATIC | `x2game=54000000(f32 1.17709071e-43)` | `crates\aa-client\src\port\combat_anim\entry.rs:35` |
| `39ac7424` | STATIC | `x2game=10270000(f32 1.40129846e-41)` | `crates\aa-client\src\port\combat_anim\entry.rs:52` |
| `39ac74ac` | STATIC | `x2game=01000000(f32 1.40129846e-45)` | `crates\aa-client\src\port\combat\targeting.rs:32` |
| `39ac7688` | STATIC | `x2game=2e000000(f32 6.44597294e-44)` | `crates\aa-client\src\port\combat_anim\entry.rs:33` |
| `39ac769c` | STATIC | `x2game=02000000(f32 2.80259693e-45)` | `crates\aa-client\src\port\combat\front.rs:12` |
| `39ac7820` | STATIC | `x2game=f22a0000(f32 1.54058753e-41)` | `crates\aa-client\src\port\interaction\action.rs:21` |
| `39ac783c` | STATIC | `x2game=f22a0000(f32 1.54058753e-41)` | `crates\aa-client\src\port\interaction\doodad.rs:15` |
| `39ac7848` | STATIC | `x2game=df360000(f32 1.96840395e-41)` | `crates\aa-client\src\port\interaction\doodad.rs:13` |
| `39ac785c` | STATIC | `x2game=a3190000(f32 9.19672182e-42)` | `crates\aa-client\src\port\interaction\doodad.rs:78` |
| `39ac7914` | STATIC | `x2game=31000000(f32 6.86636248e-44)` | `crates\aa-client\src\port\interaction\builders.rs:324` |
| `39ac7930` | STATIC | `x2game=3a030000(f32 1.15747253e-42)` | `crates\aa-client\src\port\interaction\doodad.rs:17` |
| `39ac793c` | STATIC | `x2game=2f2f0000(f32 1.69262842e-41)` | `crates\aa-client\src\port\interaction\npc.rs:26` |
| `39ac7940` | STATIC | `x2game=df360000(f32 1.96840395e-41)` | `crates\aa-client\src\port\interaction\npc.rs:24` |
| `39ac794c` | STATIC | `x2game=91320000(f32 1.81398086e-41)` | `crates\aa-client\src\port\interaction\npc.rs:22` |
| `39ac7950` | STATIC | `x2game=332f0000(f32 1.69318893e-41)` | `crates\aa-client\src\port\interaction\npc.rs:14` |
| `39ac7954` | STATIC | `x2game=b6330000(f32 1.85503891e-41)` | `crates\aa-client\src\port\interaction\npc.rs:16` |
| `39ac7958` | STATIC | `x2game=2c340000(f32 1.87157423e-41)` | `crates\aa-client\src\port\interaction\npc.rs:18` |
| `39ac795c` | STATIC | `x2game=35360000(f32 1.94458188e-41)` | `crates\aa-client\src\port\interaction\npc.rs:20` |
| `39ac796c` | STATIC | `x2game=f22a0000(f32 1.54058753e-41)` | `crates\aa-client\src\port\interaction\npc.rs:28` |
| `39ac7978` | STATIC | `x2game=2d2f0000(f32 1.69234816e-41)` | `crates\aa-client\src\port\interaction\npc.rs:30` |
| `39ac799c` | STATIC | `x2game=392f0000(f32 1.69402971e-41)` | `crates\aa-client\src\port\interaction\npc.rs:258` |
| `39ac79a0` | STATIC | `x2game=3a2f0000(f32 1.69416984e-41)` | `crates\aa-client\src\port\interaction\npc.rs:258` |
| `39ac79a4` | STATIC | `x2game=3c2f0000(f32 1.6944501e-41)` | `crates\aa-client\src\port\interaction\npc.rs:258` |
| `39ac79b0` | STATIC | `x2game=74310000(f32 1.77404386e-41)` | `crates\aa-client\src\port\interaction\npc.rs:32` |
| `39ac79c8` | STATIC | `x2game=35360000(f32 1.94458188e-41)` | `crates\aa-client\src\port\interaction\npc.rs:20` |
| `39ac79fc` | STATIC | `x2game=00001041(f32 9)` | `crates\aa-client\src\port\interaction\range.rs:40` |
| `39ac7a00` | STATIC | `x2game=00004441(f32 12.25)` | `crates\aa-client\src\port\interaction\range.rs:42` |
| `39adcc24` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\housing\decor.rs:60` |
| `39add3c0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\housing\seeds.rs:263` |
| `39ae4e80` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\remote\sample.rs:56` |
| `39ae4e8c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\remote\controller.rs:12` |
| `39ae4e90` | LIVE | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\vehicles\apply.rs:107` |
| `39ae4eb8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\anim\mount.rs:271` |
| `39ae6c64` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\anim\camera.rs:5` |
| `39ae6c68` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\anim\camera.rs:5` |
| `39aefb98` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\collision.rs:41` |
| `39aefb9c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:12` |
| `39aefbf0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:16` |
| `39aefc08` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:14` |
| `39af92c8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\movement\quadruped.rs:259` |
| `39afd348` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat_anim\entry.rs:17` |
| `39afd370` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat_anim\entry.rs:17` |
| `39afd374` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat_anim\entry.rs:17` |
| `39afe1c8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:49` |
| `39afe1cc` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:29` |
| `39afe1d8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\look.rs:4` |
| `39afe1dc` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:21` |
| `39afe1e0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:23` |
| `39afe204` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:33` |
| `39afe208` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:35` |
| `39afe20c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\collision.rs:116` |
| `39afe210` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:53` |
| `39afe214` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:57` |
| `39afe218` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:55` |
| `39afe21c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:59` |
| `39afe230` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\look.rs:5` |
| `39afe234` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:39` |
| `39afe238` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat_anim\camera.rs:34` |
| `39afe248` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:43` |
| `39afe24c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\fade.rs:36` |
| `39afe288` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\look.rs:6` |
| `39afe28c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:25` |
| `39afe290` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:27` |
| `39afe294` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:47` |
| `39afe298` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\state.rs:41` |
| `39afe2b8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\mode1.rs:216` |
| `39afe2c0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\camera\mode1.rs:62` |
| `39aff998` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\npc.rs:304` |
| `39aff9b4` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\doodad.rs:4` |
| `39aff9b8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\loot.rs:3` |
| `39affa0c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\bar.rs:1` |
| `39b02294` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\requirement.rs:240` |
| `39d29288` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\presentation.rs:140` |
| `39d44c50` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\cast.rs:31` |
| `3a167ea0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\housing\lifecycle.rs:215` |
| `3a167ebc` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\housing\lifecycle.rs:251` |
| `3a167ee8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\housing\lifecycle.rs:188` |
| `3a1682f4` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\anim\summon.rs:47` |
| `3a16830c` | STATIC | `x2game=5cb59302(f32 2.17038039e-37)` | `crates\aa-client\src\port\combat\presentation.rs:11` |
| `3a169968` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\targeting.rs:26` |
| `3a1699f8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat_anim\weapon.rs:13` |
| `3a1776e0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\token.rs:31` |
| `3a177700` | LIVE | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\front.rs:346` |
| `3a177710` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\timeline.rs:27` |
| `3a177728` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\reaction.rs:7` |
| `3a17a950` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\housing\seeds.rs:54` |
| `3a17d7cc` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\action.rs:23` |
| `3a17d7d0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\action.rs:19` |
| `3a17d7d4` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\action.rs:23` |
| `3a17e280` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\doodad.rs:199` |
| `3a17e284` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\doodad.rs:19` |
| `3a17e288` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\doodad.rs:19` |
| `3a17e28c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\doodad.rs:73` |
| `3a17e2e4` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\permission.rs:11` |
| `3a17e2e8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\permission.rs:13` |
| `3a17e6a0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\doodad.rs:7` |
| `3a17e6a4` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\doodad.rs:7` |
| `3a17e6ac` | STATIC | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\candidates.rs:72` |
| `3a17e6c4` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\builders.rs:278` |
| `3a17f52c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\npc.rs:35` |
| `3a17f530` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\npc.rs:37` |
| `3a59f858` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\movement\input_state.rs:365` |
| `3a59f864` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\movement\input_state.rs:293` |
| `3a5a4dbc` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:24` |
| `3a5a4dcc` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:6` |
| `3a5a4dd0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:28` |
| `3a5a4e68` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:157` |
| `3a5a4e78` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\click.rs:157` |
| `3a5a4ea0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\housing\seeds.rs:56` |
| `3a5aa29c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\token.rs:31` |
| `3a5aa2b4` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\token.rs:31` |
| `3a5ab31c` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\cast.rs:256` |
| `3a5ba4ac` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\interaction\loot.rs:20` |
| `3a5bb1d8` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\anim\camera.rs:6` |
| `3a5e1098` | STATIC | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\vehicles\fixed.rs:4` |
| `3a5e12f0` | RUNTIME | `x2game=00000000(f32 0)` | `crates\aa-client\src\port\combat\cooldown.rs:39` |

## Appendix B. Every vtable slot cited by port code (60)

| slot | class | first port reference |
|---|---|---|
| `0xc` | LIVE | `crates\aa-client\src\port\anim\camera.rs:14` |
| `0x0c` | STATIC | `crates\aa-client\src\port\movement\click_move.rs:57` |
| `0x10` | STATIC | `crates\aa-client\src\port\anim\mount.rs:107` |
| `0x14` | STATIC | `crates\aa-client\src\port\anim\mount.rs:105` |
| `0x18` | STATIC | `crates\aa-client\src\port\camera\mode1.rs:30` |
| `0x1c` | STATIC | `crates\aa-client\src\port\housing\lifecycle.rs:104` |
| `0x20` | STATIC | `crates\aa-client\src\port\anim\mount.rs:349` |
| `0x24` | STATIC | `crates\aa-client\src\port\combat_anim\caf.rs:7` |
| `0x28` | STATIC | `crates\aa-client\src\port\housing\lifecycle.rs:58` |
| `0x2c` | STATIC | `crates\aa-client\src\port\combat_anim\caf.rs:8` |
| `0x30` | STATIC | `crates\aa-client\src\port\combat_anim\caf.rs:7` |
| `0x34` | STATIC | `crates\aa-client\src\port\combat_anim\entry.rs:304` |
| `0x38` | STATIC | `crates\aa-client\src\port\interaction\doodad.rs:76` |
| `0x3c` | STATIC | `crates\aa-client\src\port\movement\quad_scope.rs:38` |
| `0x40` | STATIC | `crates\aa-client\src\port\movement\swim.rs:97` |
| `0x4c` | STATIC | `crates\aa-client\src\port\combat_anim\caf.rs:6` |
| `0x50` | STATIC | `crates\aa-client\src\port\combat\targeting.rs:441` |
| `0x58` | LIVE | `crates\aa-client\src\port\movement\update.rs:8` |
| `0x60` | STATIC | `crates\aa-client\src\port\anim\machine.rs:120` |
| `0x68` | STATIC | `crates\aa-client\src\port\movement\quadruped.rs:239` |
| `0x6c` | STATIC | `crates\aa-client\src\port\movement\swim.rs:55` |
| `0x70` | STATIC | `crates\aa-client\src\port\interaction\pick.rs:139` |
| `0x78` | STATIC | `crates\aa-client\src\port\combat_anim\entry.rs:308` |
| `0x7c` | STATIC | `crates\aa-client\src\port\movement\fall.rs:81` |
| `0x84` | STATIC | `crates\aa-client\src\port\movement\quad_scope.rs:39` |
| `0x8c` | STATIC | `crates\aa-client\src\port\camera\mode1.rs:30` |
| `0x90` | STATIC | `crates\aa-client\src\port\anim\command.rs:8` |
| `0x98` | STATIC | `crates\aa-client\src\port\movement\fall.rs:72` |
| `0x9c` | STATIC | `crates\aa-client\src\port\movement\quadruped.rs:257` |
| `0xa0` | STATIC | `crates\aa-client\src\port\movement\quadruped.rs:216` |
| `0xa8` | STATIC | `crates\aa-client\src\port\movement\quadruped.rs:237` |
| `0xb0` | STATIC | `crates\aa-client\src\port\movement\swim.rs:97` |
| `0xb4` | STATIC | `crates\aa-client\src\port\combat_anim\entry.rs:306` |
| `0xc0` | STATIC | `crates\aa-client\src\port\movement\swim.rs:63` |
| `0xcc` | STATIC | `crates\aa-client\src\port\camera\mode1.rs:50` |
| `0xd8` | STATIC | `crates\aa-client\src\port\cryanim\process.rs:186` |
| `0xf4` | STATIC | `crates\aa-client\src\port\combat_anim\entry.rs:339` |
| `0x108` | STATIC | `crates\aa-client\src\port\movement\request.rs:189` |
| `0x110` | STATIC | `crates\aa-client\src\port\movement\swim.rs:59` |
| `0x120` | STATIC | `crates\aa-client\src\port\combat_anim\scheduler.rs:577` |
| `0x138` | STATIC | `crates\aa-client\src\port\camera\mode1.rs:37` |
| `0x144` | STATIC | `crates\aa-client\src\port\combat_anim\hit.rs:3` |
| `0x158` | STATIC | `crates\aa-client\src\port\movement\quadruped.rs:222` |
| `0x160` | STATIC | `crates\aa-client\src\port\movement\quad_scope.rs:34` |
| `0x168` | STATIC | `crates\aa-client\src\port\combat_anim\scheduler.rs:31` |
| `0x16c` | STATIC | `crates\aa-client\src\port\interaction\pick.rs:24` |
| `0x170` | STATIC | `crates\aa-client\src\port\combat_anim\scheduler.rs:31` |
| `0x184` | STATIC | `crates\aa-client\src\port\movement\swim.rs:95` |
| `0x190` | STATIC | `crates\aa-client\src\port\movement\swim.rs:90` |
| `0x194` | STATIC | `crates\aa-client\src\port\movement\input_state.rs:329` |
| `0x1bc` | STATIC | `crates\aa-client\src\port\anim\camera.rs:6` |
| `0x1dc` | LIVE | `crates\aa-client\src\port\world\view_distance.rs:7` |
| `0x1e8` | STATIC | `crates\aa-client\src\port\camera\mode1.rs:37` |
| `0x200` | STATIC | `crates\aa-client\src\port\movement\input_state.rs:325` |
| `0x208` | STATIC | `crates\aa-client\src\port\camera\mode1.rs:28` |
| `0x228` | STATIC | `crates\aa-client\src\port\anim\camera.rs:6` |
| `0x2a8` | STATIC | `crates\aa-client\src\port\movement\quadruped.rs:756` |
| `0x2e0` | STATIC | `crates\aa-client\src\port\movement\swim.rs:93` |
| `0x30c` | STATIC | `crates\aa-client\src\port\camera\collision.rs:31` |
| `0x30741dcc` | STATIC | `crates\aa-client\src\port\anim\ag.rs:6` |

## Appendix C. Every float10 path cited by port code (121)

| function | float10 | module | module test style | first port reference |
|---|---:|---|---|---|
| `39736bd0` | 32 | vehicles | EXACT (19 assert_eq) | `crates\aa-client\src\port\vehicles\model.rs:9` |
| `39161690` | 29 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\mod.rs:9` |
| `39016fe0` | 25 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\builder.rs:7` |
| `39404d50` | 22 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\input_state.rs:3` |
| `39162d00` | 18 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\basis.rs:37` |
| `3913f020` | 14 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\rotation.rs:95` |
| `3913f710` | 14 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\rotation.rs:96` |
| `39402e00` | 14 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\turn.rs:1` |
| `390a3de0` | 13 | vehicles | EXACT (19 assert_eq) | `crates\aa-client\src\port\vehicles\controller.rs:7` |
| `390aaf30` | 13 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\mount.rs:173` |
| `390eed80` | 12 | vehicles | EXACT (19 assert_eq) | `crates\aa-client\src\port\vehicles\timeout.rs:4` |
| `3915b1c0` | 12 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\zoom.rs:1` |
| `3915c040` | 12 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\look.rs:1` |
| `39426d10` | 12 | combat | MIXED (179 exact/1 approx/1 tol) | `crates\aa-client\src\port\combat\front.rs:10` |
| `3915c700` | 11 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\look.rs:20` |
| `3915c7d0` | 11 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\look.rs:18` |
| `39165420` | 11 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\collision.rs:1` |
| `39169860` | 11 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\basis.rs:36` |
| `39327830` | 10 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\geometry.rs:3` |
| `39402690` | 10 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\input.rs:1` |
| `3912b330` | 9 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\quadruped.rs:6` |
| `3912ebc0` | 9 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\acosf.rs:2` |
| `3913ff90` | 9 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\rotation.rs:7` |
| `3915c2b0` | 9 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\mod.rs:8` |
| `39402590` | 9 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\input.rs:1` |
| `39567840` | 9 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:12` |
| `39567ed0` | 9 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:26` |
| `3903c5a0` | 8 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\decor.rs:13` |
| `390a30e0` | 8 | remote | MIXED (23 exact/4 approx/4 tol) | `crates\aa-client\src\port\remote\controller.rs:7` |
| `3915ccf0` | 8 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\basis.rs:1` |
| `3932c900` | 8 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\start.rs:22` |
| `39395760` | 8 | combat | MIXED (179 exact/1 approx/1 tol) | `crates\aa-client\src\port\combat\timeline.rs:12` |
| `3939b010` | 8 | combat | MIXED (179 exact/1 approx/1 tol) | `crates\aa-client\src\port\combat\timeline.rs:7` |
| `395676c0` | 8 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:9` |
| `39567740` | 8 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:10` |
| `395677c0` | 8 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:11` |
| `39567c90` | 8 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:20` |
| `39567d30` | 8 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:21` |
| `39567dd0` | 8 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:22` |
| `3909c660` | 7 | vehicles | EXACT (19 assert_eq) | `crates\aa-client\src\port\vehicles\controller.rs:114` |
| `39118650` | 7 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\request.rs:114` |
| `3912e540` | 7 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\quad_scope.rs:1` |
| `3913e8d0` | 7 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\rotation.rs:10` |
| `39567bd0` | 7 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:19` |
| `390c7410` | 6 | vehicles | EXACT (19 assert_eq) | `crates\aa-client\src\port\vehicles\visual.rs:3` |
| `3912d860` | 6 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\quadruped.rs:8` |
| `3932e890` | 6 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\fence.rs:10` |
| `39567b80` | 6 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:18` |
| `39569190` | 6 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:29` |
| `390367d0` | 5 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\seeds.rs:95` |
| `3911feb0` | 5 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\glide.rs:283` |
| `39156e50` | 5 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\caf.rs:3` |
| `3932ead0` | 5 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\fence.rs:9` |
| `39337fb0` | 5 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\points.rs:11` |
| `3935a2b0` | 5 | combat | MIXED (179 exact/1 approx/1 tol) | `crates\aa-client\src\port\combat\targeting.rs:84` |
| `39368520` | 5 | combat | MIXED (179 exact/1 approx/1 tol) | `crates\aa-client\src\port\combat\targeting.rs:7` |
| `3938b460` | 5 | vehicles | EXACT (19 assert_eq) | `crates\aa-client\src\port\vehicles\codec.rs:9` |
| `3942c640` | 5 | interaction | MIXED (74 exact/0 approx/1 tol) | `crates\aa-client\src\port\interaction\pick.rs:2` |
| `39567950` | 5 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:23` |
| `395679e0` | 5 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:24` |
| `39567a70` | 5 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:14` |
| `39567e70` | 5 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:25` |
| `39567f70` | 5 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:28` |
| `39569540` | 5 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:30` |
| `3989bf90` | 5 | interaction | MIXED (74 exact/0 approx/1 tol) | `crates\aa-client\src\port\interaction\range.rs:5` |
| `39113800` | 4 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\quadruped.rs:231` |
| `39157130` | 4 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\entry.rs:335` |
| `3922aed0` | 4 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\clamp.rs:1` |
| `39332030` | 4 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\lifecycle.rs:11` |
| `3933be30` | 4 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\side.rs:132` |
| `39354670` | 4 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\entry.rs:294` |
| `39358c00` | 4 | interaction | MIXED (74 exact/0 approx/1 tol) | `crates\aa-client\src\port\interaction\pick.rs:110` |
| `393f0a50` | 4 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\mod.rs:18` |
| `394eaac0` | 4 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\decor.rs:27` |
| `39567ab0` | 4 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:15` |
| `39567af0` | 4 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:16` |
| `39567b30` | 4 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:17` |
| `39567f20` | 4 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:27` |
| `3903e020` | 3 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\decor.rs:25` |
| `3909b070` | 3 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\mount.rs:180` |
| `3909b740` | 3 | remote | MIXED (23 exact/4 approx/4 tol) | `crates\aa-client\src\port\remote\controller.rs:21` |
| `390a5f00` | 3 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\mount.rs:95` |
| `390aa730` | 3 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\mount.rs:8` |
| `390af040` | 3 | remote | MIXED (23 exact/4 approx/4 tol) | `crates\aa-client\src\port\remote\controller.rs:22` |
| `390b2af0` | 3 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\click_move.rs:242` |
| `390dcc80` | 3 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\camera.rs:3` |
| `390dd080` | 3 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\camera.rs:1` |
| `39112690` | 3 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\direction.rs:1` |
| `39120300` | 3 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\swim.rs:1` |
| `391545e0` | 3 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\scheduler.rs:16` |
| `39154ad0` | 3 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\scheduler.rs:562` |
| `39156160` | 3 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\scheduler.rs:13` |
| `391694c0` | 3 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\fade.rs:1` |
| `3916aab0` | 3 | camera | MIXED (6 exact/0 approx/1 tol) | `crates\aa-client\src\port\camera\mod.rs:6` |
| `3930b5b0` | 3 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\breath.rs:100` |
| `39328630` | 3 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\mapwalk.rs:3` |
| `39402c50` | 3 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\input_state.rs:31` |
| `39567fb0` | 3 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:8` |
| `3983b280` | 3 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\seeds.rs:91` |
| `398b5706` | 3 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\position.rs:13` |
| `39093800` | 2 | interaction | MIXED (74 exact/0 approx/1 tol) | `crates\aa-client\src\port\interaction\doodad.rs:89` |
| `3909b4a0` | 2 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\mount.rs:469` |
| `390a2e90` | 2 | anim | MIXED (146 exact/10 approx/12 tol) | `crates\aa-client\src\port\anim\mount.rs:383` |
| `390b2cf0` | 2 | remote | MIXED (23 exact/4 approx/4 tol) | `crates\aa-client\src\port\remote\controller.rs:13` |
| `390b2e30` | 2 | vehicles | EXACT (19 assert_eq) | `crates\aa-client\src\port\vehicles\apply.rs:8` |
| `39113b50` | 2 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\swim.rs:81` |
| `3911c780` | 2 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\fall.rs:1` |
| `3912ce00` | 2 | movement | MIXED (131 exact/2 approx/5 tol) | `crates\aa-client\src\port\movement\swim_control.rs:1` |
| `39157680` | 2 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\entry.rs:21` |
| `3932fe60` | 2 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\lifecycle.rs:15` |
| `39330830` | 2 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\start.rs:22` |
| `39333e20` | 2 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\areas.rs:1` |
| `39355a80` | 2 | combat_anim | EXACT (55 assert_eq) | `crates\aa-client\src\port\combat_anim\entry.rs:345` |
| `39358c70` | 2 | interaction | MIXED (74 exact/0 approx/1 tol) | `crates\aa-client\src\port\interaction\pick.rs:149` |
| `393a8bd0` | 2 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\seeds.rs:97` |
| `393a8fa0` | 2 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\seeds.rs:8` |
| `393a9690` | 2 | housing | EXACT (102 assert_eq) | `crates\aa-client\src\port\housing\ray.rs:20` |
| `394273e0` | 2 | interaction | MIXED (74 exact/0 approx/1 tol) | `crates\aa-client\src\port\interaction\pick.rs:120` |
| `39428d80` | 2 | interaction | MIXED (74 exact/0 approx/1 tol) | `crates\aa-client\src\port\interaction\pick.rs:1` |
| `39566d00` | 2 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:7` |
| `395678d0` | 2 | formulas | ? | `crates\aa-client\src\port\formulas\eval.rs:13` |
