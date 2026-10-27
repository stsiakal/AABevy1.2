# Function inventory (generated from funcs.csv / calls.tsv)

Every address is a function VA in the 1.2 x2game image (base 0x38FF0000); sizes are from
`funcs.csv` (manual sizes were measured when Ghidra did not start the function).
Callers/callees are from `calls.tsv` (addresses only; first 10/12 shown, `+N` is the remainder).
Role/purity is from the reading in `MAP.md`, `classes.md` and `checks.md`;
`stateful` = reads/writes client/world state, `pure` = no such state.

| group | addr | size | name | role | state | callers | callees |
|---|---|---|---|---|---|---|---|
| A | 0x39016fe0 | 252 | FUN_39016fe0 | wheel/elevation rotation delta (builder_rotate_angle, 6 actions) | stateful | 3903e020,39332030,393a9690,394d73e0 | 39424580,39424580,39424580,39424580,39424580,39424580 |
| A | 0x39017410 | 316 | FUN_39017410 | cursor image + BUILDER_STEP 0x224 state/event | stateful | 3932a3b0,3932a3b0,393a9830,394d7510 | 394245a0,39424240,3922b1c0,3921aa80,3922b1c0,397a7b40,3921aa80 |
| A | 0x3908c810 | 29 | FUN_3908c810 | support function (see group notes) | pure | 39330ba0,39331760,39332200,39334ab0,39336520 | 39897070 |
| A | 0x3908c850 | 23 | FUN_3908c850 | housing desc +0xd flag (dot-style plot) | pure | 3933a300,3933a380 | 3963b390 |
| A | 0x3921ac90 | 6 | FUN_3921ac90 | mouse position getter (returns 2 floats) | stateful | 3903e020,390f3630,390f38f0,390f38f0,3917dd80,3917e110,3927a110,3927cb20,3927cd80,39292bc0 +18 |  |
| A | 0x39325160 | 33 | FUN_39325160 | set temp unit position (vtable+0x44) | stateful | 39332100,39332180,393323c0,39336600,39337320,393373a0,39337840,393378b0,39337fb0,39338d40 +13 | 39360020 |
| A | 0x39325f60 | 176 | FUN_39325f60 | CSConstructHouseTax 0x56 | stateful | 39332200 | 39428370,3908c870,3961dd70,397abc50,39186810 |
| A | 0x39326800 | 168 | FUN_39326800 | set temp unit yaw (vtable+0xc) | stateful | 393323c0,39337320,393373a0,39337840,393378b0,39339580,393396a0,39339ab0,39339ab0,3933a380 +6 | 39360020,3908c870 |
| A | 0x39326b40 | 205 | FUN_39326b40 | point-in-house/garden test | stateful | 390367d0,390367d0,3903c5a0,39335860,393a4cc0,393a7170 | 39326980 |
| A | 0x39326c10 | 889 | FUN_39326c10 | housing area/height helpers | stateful | 39332180 |  |
| A | 0x39327830 | 212 | FUN_39327830 | distance to house/connector box | stateful | 39328630 |  |
| A | 0x39328630 | 386 | FUN_39328630 | find nearest house around point | stateful | 390367d0,390367d0,3932a190,393a4cc0,393a7170,394797c0,39479890,3947a040,3947a310,394eaac0 | 39326980,39327830 |
| A | 0x393287c0 | 500 | FUN_393287c0 | box-vs-box overlap | stateful | 393a8fa0 | 39326980 |
| A | 0x393289c0 | 1719 | FUN_393289c0 | neighbour-overlap gather | stateful | 3932aa00,3932aa00 | 39360020,390b3750,3908d6c0,39360020,390b3750,3908d6c0 |
| A | 0x3932a270 | 140 | FUN_3932a270 | Housing::LoadConnectors profiler span; reload connector data | stateful | 390e69d0,39364db0 | 390115a0,390b3750,390904b0,390116d0 |
| A | 0x3932a3b0 | 1245 | FUN_3932a3b0 | HousingManager::Update: builder/deco cursor pump, preview spin, links | stateful | 391739e0 | 39004bb0,392aa250,392aa250,39332030,39017410,3903e020,39017410,3903d870,3908c870,39035110,39093800,39424220 +3 |
| A | 0x3932b510 | 49 | FUN_3932b510 | house lookup by tl | stateful | 390382d0,391d7890,391d7920,391d79b0,391d7a40,39336600,39337490,39338d40,39356d20,393b3200 | 3932ac80 |
| A | 0x3932b550 | 71 | FUN_3932b550 | house lookup by unit id | stateful | 39038a30,392f7940,3935bb90,393b34b0,393b36a0,393b37b0,393bbf10,393befd0,393c0040,394c2070 | 3932ac80 |
| A | 0x3932c900 | 1090 | FUN_3932c900 | init/refresh connector fences around temp house | stateful | 39330ba0 | 3932b5a0,3908c870,393294a0,393294a0 |
| A | 0x3932d0f0 | 676 | FUN_3932d0f0 | Housing::InitFromNetHouse + LoadConnectors; copies SCHouseState | stateful | 3932fa00 | 390115a0,3963b390,39360020,398970b0,39897170,390115a0,390b3750,390904b0,390116d0,3932aa00,390b39c0,3932a300 +5 |
| A | 0x3932dbb0 | 358 | FUN_3932dbb0 | query houses within radius | stateful | 39336600,39337490,39338d40 | 3908c870,3908d400,3932d730,3932d730 |
| A | 0x3932e530 | 722 | FUN_3932e530 | SCConstructHouseTax -> builder+0x21c, HOUSE_BUILD_INFO 0x100 | stateful | 391d7cb0 | 398b562a,394248b0,398b562a,394248b0,398b562a,394248b0,391e8610,390155a0,390155a0,390155a0,390155a0,390155a0 +4 |
| A | 0x3932ead0 | 940 | FUN_3932ead0 | Housing::InitFences; spawns fence doodads | stateful | 3932ee80,3932eef0 | 390115a0,390116d0,39326980,3932e890,3932e890,3932e890,3932e890,3932d9a0,3932da80,3932bdd0,390116d0 |
| A | 0x3932eef0 | 17 | FUN_3932eef0 | InitFences if this is a housing unit | stateful | 3933a300 | 3932ead0 |
| A | 0x3932ef10 | 439 | FUN_3932ef10 | pre-create server check / packet 0xfc | stateful | 3932fe60 | 394df370,39897340,39428370,39744420,3950a1b0,39327910,394f10a0,3930ffe0,3930ffe0,398b53e0 |
| A | 0x3932f0d0 | 123 | FUN_3932f0d0 | iterate temp fence list | stateful | 3932f520,39330830,393a8470,393a89d0 | 3932ee80 |
| A | 0x3932f520 | 240 | FUN_3932f520 | destroy temp fence/neighbour lists | stateful | 3932f960,39330120,393304b0 | 392280f0,3950a1f0,3932f0d0 |
| A | 0x3932f6b0 | 629 | FUN_3932f6b0 | house object init from parsed net/template data | stateful | 3932fa00 | 3908c790,3950a1b0,3950a1b0,3950a1b0,3950a1b0,3908e540,3932de10,3932dec0,398b53e0,398b53e0,398b53e0 |
| A | 0x3932f960 | 154 | FUN_3932f960 | finish builder: effect, destroy temp house, cleanup | stateful | 3932fe60,3932fe60,394f18f0 | 394ef970,3950a1f0,39360020,390b3750,39326770,39366480,3932f520,390173b0 |
| A | 0x3932fa00 | 275 | FUN_3932fa00 | create Housing object, InitFromNetHouse, register it | stateful | 391d77f0,3932fb20 | 3950a1b0,3932f6b0,3932bd60,39329e30,3932d0f0,39326690,3932cdb0,3908e040 |
| A | 0x3932fb20 | 272 | FUN_3932fb20 | CreateTemporaryHouse from housings.main_model_id | stateful | 393301a0,39330ba0 | 3963b390,39368fb0,390b3750,390ffb30,3910a8a0,390ffb30,3932fa00 |
| A | 0x3932fe60 | 701 | FUN_3932fe60 | CSCreateHouse 0x57 + 5 s limit + BUILDER_END | stateful | 39332200,39477cb0 | 39424220,39428370,39428370,3908c870,3961dd70,397b09d0,3932ef10,3932f960,392e3420,398b562a,398b562a,39426f40 +5 |
| A | 0x39330120 | 108 | FUN_39330120 | cancel builder: BUILDER_END, free item, destroy temp house | stateful | 390171a0,390172f0,39017340,39330470,39330ba0,39331760,39331760 | 39424220,394ef970,3950a1f0,39366480,3932f520,390173b0 |
| A | 0x393301a0 | 202 | FUN_393301a0 | ChangeBuilderHouse: swap design | stateful | 39332860,39332980 | 3932fb20,3908c870,39366480,39360020,393268b0,39360020,390b3750,39335340 |
| A | 0x39330270 | 158 | FUN_39330270 | HOUSE_DECO_UPDATED / WAIT_REPLY_FROM_SERVER for selected house | stateful | 390382d0 | 39327770,39033b00,398978a0,3932fd00,39424220,39424220 |
| A | 0x393304b0 | 114 | FUN_393304b0 | reset temp fence/neighbour lists, resolve local player | stateful | 39330ba0 | 3932f520,39208930 |
| A | 0x39330830 | 869 | FUN_39330830 | init neighbour/link fences around temp house | stateful | 39330ba0 | 393294a0,393294a0,3901aa10,39228fe0,3950a1f0,39330530,3932f0d0 |
| A | 0x39330ba0 | 604 | FUN_39330ba0 | StartBuild: global checks, temp house, builder class by category | stateful | 394eaac0 | 39016fd0,39017620,39330120,398a7a30,39428370,393304b0,39428370,3932fb20,39360020,390b3750,39330830,3932c900 +16 |
| A | 0x39331760 | 123 | FUN_39331760 | right click: cancel / 2->1 / popup -> 2 / elevation->rotation | stateful |  | 3908c810,39330120,39424220,39330120 |
| A | 0x393317e0 | 8 | FUN_393317e0 | support function (see group notes) | stateful |  |  |
| A | 0x393317f0 | 8 | FUN_393317f0 | support function (see group notes) | stateful |  |  |
| A | 0x39331800 | 8 | FUN_39331800 | support function (see group notes) | stateful |  |  |
| A | 0x39331810 | 8 | FUN_39331810 | support function (see group notes) | stateful |  |  |
| A | 0x39331f70 | 178 | FUN_39331f70 | run callback over all connector slots | stateful | 39333e20,39334060,39334060,39334060,39334060,39334060,39334060,39334380,393349e0,393349e0 +1 |  |
| A | 0x39332030 | 202 | FUN_39332030 | per-frame state 1 ray SetPos, 2 wheel Rotate, 3 elevation, 4 revalidate | stateful | 3932a3b0 | 392aa2e0,3921ac90,394298c0,393fd240,39016fe0 |
| A | 0x39332100 | 115 | FUN_39332100 | garden plot/snap helper | stateful | 3933a300 | 39325e30,39325160 |
| A | 0x39332180 | 124 | FUN_39332180 | simple 6 m terrain spread check, commit position | stateful | 3933a300,3933a380 | 3908c870,3908d400,39326c10,3908c870,39325160 |
| A | 0x39332200 | 208 | FUN_39332200 | left click: state 1->2, house 2->quote, others -> elevation/create | stateful | 393327e0,39332ad0 | 3908c810,39325f60,398970e0,398b562a,394df370,397ccd50,3932fe60 |
| A | 0x393322d0 | 233 | FUN_393322d0 | connector/plot grid for garden designs | stateful |  | 39325190,39325e30,39331950 |
| A | 0x393323c0 | 714 | FUN_393323c0 | initial yaw/rotation setup [I] | stateful | 39337320,393373a0 | 3908c870,3908c870,39325160,39326800 |
| A | 0x39332690 | 101 | FUN_39332690 | orientation/step helper [I] | stateful |  | 3908d6c0 |
| A | 0x39332700 | 101 | FUN_39332700 | orientation/step helper [I] | stateful |  | 3908d6c0 |
| A | 0x39332770 | 101 | FUN_39332770 | orientation/step helper [I] | stateful |  | 3908d6c0 |
| A | 0x393327e0 | 128 | FUN_393327e0 | keyboard step + confirm [I] | stateful |  | 3908d6c0,3908cb30,39332200 |
| A | 0x39332ad0 | 128 | FUN_39332ad0 | keyboard step + confirm [I] | stateful |  | 3908d6c0,3908cae0,39332200 |
| A | 0x39333cb0 | 134 | FUN_39333cb0 | gather player/target position and garden class | stateful | 39335860,393363b0 | 39360020,390b3850,39325190,3908d400,3908ce50,39326980,393312a0,3908d400,3908ce50 |
| A | 0x39333d40 | 214 | FUN_39333d40 | range check (unit + 30 m) -> 0x9a | stateful | 393363b0 | 390b3850,3963b390 |
| A | 0x39333e20 | 573 | FUN_39333e20 | housing area/category membership -> 0x244/0x70 | stateful | 393363b0 | 39897070,39897280,39331f70,39017100,39017550 |
| A | 0x39334060 | 799 | FUN_39334060 | dominion/guard-house -> 0x154/0x155 | stateful | 393363b0 | 398970e0,392f34d0,39331f70,39331f70,392f80e0,39331f70,3961de30,39333860,392f34d0,397a6c60,397f2040,39331f70 +5 |
| A | 0x39334380 | 218 | FUN_39334380 | water/land-only -> 0x17f/0x180 | stateful | 393363b0 | 39331f70 |
| A | 0x39334460 | 216 | FUN_39334460 | debug error text + invalid material override | stateful | 393364e0,39337320,393373a0,39337450,39337840,393378b0,39339580,39339960,3933a300,3933a380 +5 | 39017100,3900c0f0,39017550,39327b50 |
| A | 0x39334540 | 502 | FUN_39334540 | height band -> 0x17c | stateful | 39337320,393373a0,39338d40,3933a040,3933a400 | 3908d6c0 |
| A | 0x39334740 | 619 | FUN_39334740 | 20 m grid draw for non-house builders | stateful | 39337490 |  |
| A | 0x393349e0 | 203 | FUN_393349e0 | OBB-vs-OBB/interval intersect helper | stateful | 39335860 | 39331f70,39331f70,39332c10,390318f0 |
| A | 0x393349c0 | 12 | FUN_393349c0 | wall/gate helper: toggle +0x238, jmp 39332980 (not in funcs.csv) | stateful |  | 39332980 |
| A | 0x39334ab0 | 1190 | FUN_39334ab0 | kind-1 plot draw: white points 0xAFFFFFFF + outline boxes | stateful | 39337490 | 3908c810,39360020,390b3750,390b3870,3908c870,39331840,3908c870,39331840 |
| A | 0x393354c0 | 566 | FUN_393354c0 | neighbour set difference and fence destroy | stateful | 393363b0 | 39035110,393a41b0,39035110,393a4cc0,39035e90,39334ff0 |
| A | 0x39335860 | 2234 | FUN_39335860 | OBB overlap vs houses/units -> 0x71/0x72 | stateful | 393363b0 | 3908ccb0,39333cb0,393349e0,39360020,390b3850,393352d0,390b3750,3910a950,3963b390,39164600,390c9f40,39164600 +22 |
| A | 0x39336140 | 324 | FUN_39336140 | HouseBuilder ctor: vtable 0x399cf920, temp unit, zero 0x1e4 | stateful | 39330ba0,39336520 | 3950a1b0,398b562a,3963b390,39002bf0,398b5446,398b562a,398b53e0 |
| A | 0x39336290 | 16 | FUN_39336290 | elevation stub (ret 4) | stateful |  |  |
| A | 0x393363b0 | 280 | FUN_393363b0 | validation entry; ordered checks, first error wins | stateful | 393364e0,39337320,393373a0,39337450,39337840,393378b0,39339580,39339960,3933a300,3933a380 +5 | 39333cb0,39333d40,39333e20,39334060,39334380,3950a1b0,39335860,393354c0,39035e90,3950a1f0,398b53e0 |
| A | 0x393364e0 | 51 | FUN_393364e0 | reset state, revalidate, tint | stateful |  | 398b562a,393363b0,39334460 |
| A | 0x39336520 | 202 | FUN_39336520 | default (Castle/Keep) ctor | stateful | 39330ba0,393377d0,39337800,39337920 | 39336140,3908c810 |
| A | 0x393365f0 | 16 | FUN_393365f0 | vtable stub (ret) | stateful |  |  |
| A | 0x39336600 | 3341 | FUN_39336600 | stair/wall local placement/snap logic | stateful | 39337840,393378b0 | 3908c870,3908d400,3908d6c0,3932dbb0,3932b510,3908d6c0,3908d6c0,3963b390,3908d6c0,39093ef0,398b577c,3903b820 +7 |
| A | 0x39337320 | 113 | FUN_39337320 | default class SetPos (snap + height) | stateful |  | 398b562a,39325160,39326800,393323c0,39334540,393363b0,39334460 |
| A | 0x393373a0 | 161 | FUN_393373a0 | default class Rotate | stateful |  | 398b562a,39325160,39326800,393323c0,39334540,393363b0,39334460 |
| A | 0x39337450 | 52 | FUN_39337450 | default Revalidate | stateful |  | 398b562a,393363b0,39334460 |
| A | 0x39337490 | 823 | FUN_39337490 | Render: plot/grid/connector draw | stateful | 393379a0 | 39334ab0,39333330,390b3850,39334740,3908d400,3908d6c0,3932dbb0,3932b510,39333330,3950a1f0 |
| A | 0x393377d0 | 34 | FUN_393377d0 | Tower builder ctor | stateful | 39330ba0 | 39336520 |
| A | 0x39337800 | 34 | FUN_39337800 | Stair builder ctor | stateful | 39330ba0 | 39336520 |
| A | 0x39337830 | 16 | FUN_39337830 | stair elevation stub (ret 4) | stateful |  |  |
| A | 0x39337840 | 97 | FUN_39337840 | stair SetPos | stateful |  | 398b562a,39325160,39326800,39336600,393363b0,39334460 |
| A | 0x393378b0 | 105 | FUN_393378b0 | stair Rotate | stateful |  | 398b562a,39325160,39326800,39336600,393363b0,39334460 |
| A | 0x39337920 | 121 | FUN_39337920 | Wall/gate builder ctor | stateful | 39330ba0 | 39336520 |
| A | 0x393379a0 | 1355 | FUN_393379a0 | wall/gate Render/update (vtable +0x04) | stateful |  | 39337490,3908c870,3908c870 |
| A | 0x39337ef0 | 139 | FUN_39337ef0 | bounds/vector helper used by connector iteration | stateful | 39338d40 | 39335760,39335760 |
| A | 0x39337f80 | 30 | FUN_39337f80 | HouseBuilder deleting destructor | stateful |  | 393362a0,3950a1f0 |
| A | 0x39337fb0 | 3415 | FUN_39337fb0 | kind-1 plot: rotated terrain grid, heights, +0x1fc vector; 6 m -> 0x74 | stateful | 3933a300,3933a380 | 39360020,390b3750,390b3870,3908c870,39008920,39008800,39008800,39325160 |
| A | 0x39338d10 | 36 | FUN_39338d10 | base deleting destructor | stateful |  | 393362a0,3950a1f0 |
| A | 0x39338d40 | 2047 | FUN_39338d40 | connector snap/check: nearest connector Z, 0x75/0x76, +0x220 | stateful | 39339580,393396a0,39339960,39339ab0,3933a040,3933a040,3933a040,3933a400,3933a5a0 | 3908d6c0,3932dbb0,3932b510,3908d930,3908d6c0,3908d6c0,39337ef0,3908d6c0,3908d6c0,3908d6c0,39325160,39334540 +2 |
| A | 0x39339550 | 36 | FUN_39339550 | tower deleting destructor | stateful |  | 393362a0,3950a1f0 |
| A | 0x39339580 | 284 | FUN_39339580 | tower elevation/revalidate | stateful |  | 398b562a,39325160,39326800,39338d40,393363b0,39334460 |
| A | 0x39339900 | 36 | FUN_39339900 | stair deleting destructor | stateful |  | 393362a0,3950a1f0 |
| A | 0x39339930 | 36 | FUN_39339930 | wall deleting destructor | stateful |  | 393362a0,3950a1f0 |
| A | 0x39339960 | 329 | FUN_39339960 | wall elevation/revalidate | stateful |  | 398b562a,3908c870,39325160,39332980,39338d40,393363b0,39334460 |
| A | 0x3933a040 | 689 | FUN_3933a040 | Wall/gate alternative SetPos (spawn pos + connector snap) | stateful |  | 3908c870,3908c870,3908c870,3908c870,39325160,398b562a,39334540,39338d40,3908c870,39325160,39332980,398b562a +5 |
| A | 0x3933a300 | 122 | FUN_3933a300 | HouseBuilder SetPos: move unit, dots, validate, tint | stateful |  | 398b562a,39325160,39325190,39332100,3908c850,39337fb0,39332180,3932eef0,393363b0,39334460 |
| A | 0x3933a380 | 124 | FUN_3933a380 | HouseBuilder Rotate: yaw, dots, validate, tint | stateful |  | 398b562a,39326800,3908c850,39337fb0,393363b0,39334460,39332180,393363b0,39334460 |
| A | 0x3933a400 | 412 | FUN_3933a400 | alternative SetPos (tower/stair/castle): connector snap + height | stateful |  | 3908c870,3908c870,3908c870,3908c870,39325160,398b562a,39334540,39338d40,3908c870,39325160 |
| A | 0x3933a5a0 | 282 | FUN_3933a5a0 | Tower SetPos with connector snap | stateful |  | 398b562a,39325160,39326800,393396a0,39326800,398b562a,39338d40,39326800,393363b0,39334460 |
| A | 0x3933a6c0 | 114 | FUN_3933a6c0 | Tower Rotate | stateful |  | 398b562a,39325160,39326800,393396a0,393363b0,39334460 |
| A | 0x3933a740 | 106 | FUN_3933a740 | Wall SetPos with connector snap | stateful |  | 398b562a,39325160,39326800,39339ab0,393363b0,39334460 |
| A | 0x3933a7b0 | 145 | FUN_3933a7b0 | Wall Rotate | stateful |  | 398b562a,39325160,39326800,39339ab0,393363b0,39334460 |
| A | 0x39368fb0 | 258 | FUN_39368fb0 | create client-only unit/prefab (temp housing) | stateful | 3930d570,3930da60,3930ea60,3930ea60,3932fb20,393490a0,394d89d0 | 3935ffb0,3950a1b0,39368780,3950a1b0,39367d30,393661c0,39362450,3935d130 |
| A | 0x393fe030 | 197 | FUN_393fe030 | hotkey mode switch (builder_rotate_mode on/off) | stateful | 39017220,39017220,390173b0,39161540,392045f0,392045f0,3949b190,3949b1c0 | 3950a1f0,393fabc0,39202fb0,39202db0 |
| A | 0x394298c0 | 605 | FUN_394298c0 | screen point -> terrain ray (shared by all placement cursors) | stateful | 39332030,39349ad0,393a9690,394d73e0 | 39086370 |
| A | 0x397abc50 | 45 | FUN_397abc50 | CSConstructHouseTax ctor | stateful | 39325f60 |  |
| A | 0x397b09d0 | 94 | FUN_397b09d0 | CSCreateHouse ctor | stateful | 39329080,3932fe60 |  |
| A | 0x39897070 | 34 | FUN_39897070 | support function (see group notes) | pure | 3908c810,3908ccb0,3908d930,3908d930,39333e20 |  |
| B | 0x3910a950 | 204 | FUN_3910a950 | append unit skill vector to dynamic actions | stateful | 390bde30,390be050,390ecc80,39181660,39335860,39335860,393488d0,39348bf0,3934df10,393bcb90 +2 | 390ac0e0,390a8cb0,390a8cb0 |
| B | 0x39327990 | 327 | FUN_39327990 | SCHouseBuildProgress handler; all/current, effects, event 0x103 | stateful | 391d7890 | 39360020,390b4ce0,3989f890,390b3750,390ba9c0,39326770,393255d0,3956db10,39424240 |
| B | 0x3932e210 | 786 | FUN_3932e210 | SCHouseTaxInfo -> HOUSE_TAX_INFO 0xff | stateful | 391d7c20 | 398b562a,394248b0,398b562a,394248b0,394245c0,3921a9f0,39429b20,394245f0,390155a0,390155a0,390155a0,390155a0 +6 |
| B | 0x393b1de0 | 202 | FUN_393b1de0 | ExecuteDynamicAction(i): cast a listed skill | stateful | 393b2b70,39440850 | 393bc7e0,393bc8c0,39035110,393b3ec0,39360020,393be280 |
| B | 0x393bcb90 | 134 | FUN_393bcb90 | build dynamic action list for controlled unit | stateful | 39347fb0,393bcc20,393bcc20,393bcc20,393bcc20,393bcd60,393bd850 | 390b3750,3910a950,390ac0e0 |
| B | 0x393befd0 | 442 | FUN_393befd0 | house interaction: 3 m, CSRequestHouseTax 0x5c, event 0x6d | stateful | 393b8400,393c0040 | 39093800,39093860,3932b550,3963b390,397abcd0,39186810,3956db10,39424280,390011f0,390011f0,390011f0 |
| B | 0x394784f0 | 217 | FUN_394784f0 | GetHouseRemainProgress = 0x140-0x144 | stateful |  | 39460110 |
| B | 0x394785d0 | 211 | FUN_394785d0 | GetHouseBaseProgress = house+0x140 | stateful |  | 39460110 |
| B | 0x39479460 | 380 | FUN_39479460 | append one construction-step row | stateful | 3947b3b0,3947b3b0,3947b7e0,3947b7e0,3947bc30,3947bc30 |  |
| B | 0x3947b3b0 | 1065 | FUN_3947b3b0 | GetHouseConstructionStepInfo rows | stateful |  | 39460110,3963b390,39058b50,396f6390,3947b230,39479460,3941a130,39421e20,3950a1f0,39479460 |
| C | 0x3903b820 | 107 | FUN_3903b820 | debug primitive draw wrapper | stateful | 3903b930,3903c5a0,39058580,390fe3d0,3915dc40,3917dae0,391ef0d0,39336600,393503e0,39353c50 +1 |  |
| C | 0x3903b8a0 | 130 | FUN_3903b8a0 | deco cursor cleanup (skill, cursor, cooldown, ghost) | stateful | 39325210,39330e20 | 3932ee80,390173b0 |
| C | 0x3903b9d0 | 33 | FUN_3903b9d0 | deco confirm: position->rotation, rotation->send | stateful |  |  |
| C | 0x3903ba00 | 25 | FUN_3903ba00 | deco back: rotation->position else cancel | stateful |  |  |
| C | 0x3903ba20 | 841 | FUN_3903ba20 | axis-angle -> 3x3 matrix | stateful | 3903da00,3903da00,393e4a70 |  |
| C | 0x3903bf60 | 892 | FUN_3903bf60 | decoration cursor ctor (0x54) | stateful | 3932c120 | 3963b3d0,398b577c,3932ee80 |
| C | 0x3903c2e0 | 691 | FUN_3903c2e0 | deco preview box corners | stateful | 3903c5a0,3903d870 |  |
| C | 0x3903c5a0 | 4339 | FUN_3903c5a0 | deco validation: surface/garden/house/overlap | stateful | 3903da00,3903e020 | 3963b3d0,39326b40,3903c2e0,39326ab0,3908d400,3908c870,3917f4c0,3903b820 |
| C | 0x3903d6a0 | 454 | FUN_3903d6a0 | apply deco transform + invalid material | stateful | 3903da00,3903e020 | 3903bd80 |
| C | 0x3903d870 | 388 | FUN_3903d870 | draw deco preview box | stateful | 3932a3b0 | 3903c2e0 |
| C | 0x3903da00 | 1565 | FUN_3903da00 | deco position from mouse/ray; init transform/surface | stateful | 3903e020 | 3963b3d0,3917f4c0,3903bf20,39428ba0,39035110,3903ba20,3903ba20,390302f0,3903b630,3903b930,3903c5a0,3903d6a0 |
| C | 0x3903e020 | 90 | FUN_3903e020 | deco per-frame state update | stateful | 3932a3b0 | 392aa2e0,3921ac90,3903da00,39016fe0,3903c5a0,3903d6a0 |
| C | 0x3903e080 | 37 | FUN_3903e080 | deco cursor slot helper | stateful | 39060d60 |  |
| C | 0x39327bc0 | 141 | FUN_39327bc0 | deco actability group/count check | stateful | 39327c50 | 3963b3d0,3963b350,39897400 |
| C | 0x39327c50 | 388 | FUN_39327c50 | deco pre-checks: war/limit/item/actability/count | stateful | 3932c120 | 39360020,39428370,390b3850,392f4000,39428370,3963b390,39428370,39428370,396a6e00,39428370,39327bc0,39428370 +4 |
| C | 0x39327de0 | 2119 | FUN_39327de0 | HousingManager::DecoratorOK: CSDecorateHouse 0x58 + cleanup | stateful |  |  |
| C | 0x3932ac80 | 98 | FUN_3932ac80 | item -> housing decoration design lookup | stateful | 3932b510,3932b550,3932b620,3932b680,3932b790,3932b8a0,3932ba50,3932be00,3932be40,3932bf90 +6 |  |
| C | 0x3932c120 | 1305 | FUN_3932c120 | start deco placement: pre-checks, lookup, ghost+cursor | stateful | 394eaac0 | 39016fd0,39017620,39325210,39428370,3932ac80,39428370,39327c50,3963b3d0,390b3850,390b3870,393b03b0,3950a1b0 +6 |
| C | 0x3932ee80 | 98 | FUN_3932ee80 | set/clear decorating mode | stateful | 3903b8a0,3903bf60,3932f0d0 | 3932ead0,3932dfc0 |
| C | 0x393a5930 | 853 | FUN_393a5930 | placement ghost ctor (0x3b0) | stateful | 390382d0,393a7a10 | 3983b690,39410b40,3950a1b0,3989ca10,3963b250,393a3070,398b53e0 |
| C | 0x393b03b0 | 1423 | FUN_393b03b0 | LoadDoodadModel: choose/load ghost class | stateful | 3932c120,393a1210,393b0940 | 39004bb0,3963b290,3950a1b0,393af900,390115a0,3950a1b0,393af900,397a6c00,397a6ba0,3900a220,393acf00,39002b90 +24 |
| C | 0x397ae590 | 81 | FUN_397ae590 | CSDecorateHouse ctor | stateful |  |  |
| D | 0x3908c870 | 72 | FUN_3908c870 | read player position (24 B) | stateful | 3903c5a0,3908ccb0,3908ccb0,3908ccb0,3908d930,3908d930,3908d930,3908d930,3908d930,3908d930 +41 | 390b3850 |
| D | 0x393a1050 | 32 | FUN_393a1050 | set seed builder position | stateful | 393a7720,393a9690 |  |
| D | 0x393a1210 | 158 | FUN_393a1210 | seed cursor init (calls 393b03b0) | stateful | 393a7a10 | 393b03b0,39411650 |
| D | 0x393a4cc0 | 1023 | FUN_393a4cc0 | leaf markers guide_leaf_r/g/y | stateful | 393354c0,393a7300,393a85a0,393a8fa0 | 3983b360,39328630,39326b40,3983b280,390de4c0,39325930,393a8540,3932baf0 |
| D | 0x393a6d40 | 6 | FUN_393a6d40 | current seed builder or null | stateful | 390171a0,390171f0,39017220,39017220,39017220,39017220,390172f0,39017340 |  |
| D | 0x393a6d50 | 25 | FUN_393a6d50 | seed position pointer | stateful | 393a9690 |  |
| D | 0x393a7110 | 88 | FUN_393a7110 | seed update helper | stateful | 393a9690 | 393a1a10 |
| D | 0x393a7170 | 188 | FUN_393a7170 | farm/area permission test -> 0x114 | stateful | 393a8fa0 | 39328630,39326b40,39325930 |
| D | 0x393a7300 | 148 | FUN_393a7300 | seed ghost refresh | stateful | 393a9690,393a9690 | 394d0b90,39017100,3956b860,39017550,393a4cc0 |
| D | 0x393a73a0 | 882 | FUN_393a73a0 | terrain height lookup | stateful | 393a7720 |  |
| D | 0x393a7720 | 184 | FUN_393a7720 | height-difference test -> 0x117 | stateful | 393a7dc0,393a8fa0 | 3983b2f0,3983b320,3983b340,393a73a0,393a1050 |
| D | 0x393a7a10 | 941 | FUN_393a7a10 | build seed ghost in front of player, init cursor | stateful | 3939ffa0,393a9540 | 390b3850,390b3870,3963b290,3918fab0,3950a1b0,393a5930,393a1210 |
| D | 0x393a7dc0 | 350 | FUN_393a7dc0 | seed ghost update/draw | stateful | 393a9830 | 394d0b90,393a7720,393a7060,393a6f50,39152bc0 |
| D | 0x393a7f20 | 269 | FUN_393a7f20 | seed ghost rotate | stateful | 393a9690 | 398b577c,393a2ad0 |
| D | 0x393a83b0 | 102 | FUN_393a83b0 | seed target doodad lookup | stateful | 393a8fa0 |  |
| D | 0x393a8470 | 202 | FUN_393a8470 | seed builder ctor (0x54) | stateful | 393a9540 | 394ef900,394e84e0,3932f0d0,39002bf0 |
| D | 0x393a8500 | 64 | FUN_393a8500 | seed marker helper | stateful |  |  |
| D | 0x393a8540 | 91 | FUN_393a8540 | clear marker state | stateful | 393a41b0,393a4cc0,393a5390,393a89d0,393a9690 |  |
| D | 0x393a89d0 | 505 | FUN_393a89d0 | seed builder destructor/cleanup | stateful | 393a8f60,393a9340,393a9340,393a94f0,393a9540 | 3932f0d0,393a8540,3950a1f0,3950a1f0,394ef970 |
| D | 0x393a8dd0 | 379 | FUN_393a8dd0 | seed invalid-state helper | stateful | 3902e9f0,393a8fa0 | 393a87c0,393a87c0,393a87c0,393a87c0,393a82d0 |
| D | 0x393a8f60 | 35 | FUN_393a8f60 | seed post-create state | stateful | 3903a660,393a9340 | 390173b0,393a89d0,3950a1f0 |
| D | 0x393a8fa0 | 927 | FUN_393a8fa0 | seed validation: farm/area/water/range/permission/overlap | stateful | 393a9690 | 393a7170,393a8dd0,393a83b0,3983b340,3983b280,393a85a0,393a7720,390b3850,393287c0,390367d0,393a8bd0,397a6c60 +5 |
| D | 0x393a9340 | 392 | FUN_393a9340 | CSCreateDoodad 0xe6 send + finish | stateful | 393a9810 | 39424220,39424220,3915b270,39428370,390173b0,393a89d0,3950a1f0,39428370,3983b2f0,3983b340,3961dd70,397ac850 +6 |
| D | 0x393a94f0 | 55 | FUN_393a94f0 | cancel seed builder | stateful | 390171a0,390172f0,39017340 | 39424220,39424220,390173b0,393a89d0,3950a1f0 |
| D | 0x393a9540 | 312 | FUN_393a9540 | StartDoodadPlacement (seeds): cooldown, ghost, builder | stateful | 394eaac0 | 39016fd0,39017620,390173b0,393a89d0,3950a1f0,39428370,393a7a10,3950a1b0,393a8470,394df370,397d2200,39016fd0 +1 |
| D | 0x393a9690 | 275 | FUN_393a9690 | seed per-frame: ray, position, wheel, validate, markers | stateful | 393a9830 | 393a8540,392aa2e0,393a7300,39016fe0,393a7f20,3921ac90,393a6d50,394298c0,393a1050,393a8fa0,393a7300,393a7110 |
| D | 0x393a9830 | 122 | FUN_393a9830 | seed cursor update + draw | stateful | 39038180 | 392aa250,392aa250,393a9690,39017410,393a7dc0 |
| D | 0x3961dd70 | 85 | FUN_3961dd70 | float position -> i64 fixed + origin (0x56/0x57/0xe6) | pure | 39007600,390353b0,39037060,39094680,390964e0,3909c660,3909f4b0,390abe10,390ad7e0,390f38f0 +26 | 398b5706,398b5706 |
| D | 0x397ac850 | 77 | FUN_397ac850 | CSCreateDoodad ctor | stateful | 393a9340 |  |
| E | 0x3956b860 | 5512 | FUN_3956b860 | error code -> key string (783 cases) | pure | 3900c0f0,391e4f00,392f3da0,393a7300,393bac20,394d3090 |  |
| U | 0x39325df0 | 51 | FUN_39325df0 | SCHousePermissionChanged handler | stateful | 391d7920 | 39424220 |
| U | 0x3932b680 | 258 | FUN_3932b680 | SCHouseSetForSale handler | stateful | 391d7dc0 | 3932ac80,39325660,39424240,39424220 |
| U | 0x3932b790 | 262 | FUN_3932b790 | SCHouseResetForSale handler | stateful | 391d7e50 | 3932ac80,39325660,39424240,39424220 |
| U | 0x3932b8a0 | 432 | FUN_3932b8a0 | SCHouseSold handler | stateful | 391d7ed0 | 3932ac80,397abcd0,39186810,39424220,39424240,39424240 |
| U | 0x3932be00 | 64 | FUN_3932be00 | CSAllowHousingRecover toggle | stateful | 391d7d40 | 3932ac80 |
| U | 0x3932be40 | 331 | FUN_3932be40 | house rename validation + CSChangeHouseName | stateful | 39477c30 | 3932ac80,39325660,39428370,398b562a,398b562a,397abc80,39186810 |
| U | 0x3932bf90 | 258 | FUN_3932bf90 | permission validation + CSChangeHousePermission | stateful | 39477c70 | 3932ac80,3963b390,39428370,39325660,392ff2d0,39300000,39428370,392f80e0,39428370,397abcb0,39186810 |
| U | 0x3932c640 | 444 | FUN_3932c640 | sale validation + CSSellHouse | stateful | 394799c0 | 3932ac80,39325660,39428370,398970e0,39428370,39428370,39428370,3963b390,39428370,395720e0,39897100,394df370 +5 |
| U | 0x3932c800 | 164 | FUN_3932c800 | CSSellHouseCancel | stateful | 39477d00 | 3932ac80,39325660,39428370,39428370,397abcf0,39186810 |
| U | 0x3932e020 | 486 | FUN_3932e020 | CSBuyHouse | stateful | 39477d30,394f18f0 | 3932ac80,39325660,39428370,39428370,390b4990,39428370,39744420,3930ff10,394f10a0,3930ffe0,397b0a90,39186810 +1 |
| U | 0x39477cb0 | 44 | FUN_39477cb0 | Build NextStepFromBuildCheck -> CSCreateHouse | stateful |  | 394df370,397ccd50,3932fe60 |
| U | 0x39477ce0 | 31 | FUN_39477ce0 | Build PrevStepFromBuildCheck -> step back | stateful |  | 39016fd0,39017220 |
| U | 0x39477d80 | 116 | FUN_39477d80 | furniture-recover checkbox -> CSAllowHousingRecover | stateful |  | 39460110,397abce0,39186810 |
| U | 0x39478740 | 145 | FUN_39478740 | Demolish: casts skill 11404 | stateful |  | 39360020,39397e50 |
| U | 0x39478cd0 | 169 | FUN_39478cd0 | CountTaxItemInBag (31891+31892) | stateful |  | 394df370,397ccda0,397ccda0 |
| U | 0x39478d80 | 162 | FUN_39478d80 | CountTaxItemForTax(money) | stateful |  | 394248e0,398972e0 |
| U | 0x394796b0 | 102 | FUN_394796b0 | GetHouseDecoCount | stateful |  | 39460110,3963b390,39058c40 |
| U | 0x394797c0 | 194 | FUN_394797c0 | GetActabilityDecoCount | stateful |  | 390b3850,39328630,39325ec0 |
| U | 0x39479890 | 295 | FUN_39479890 | GetDecoActabilityByType | stateful |  | 390b3850,39328630,39325ee0,397f22d0,39001cc0,390309f0,39058b50 |
| U | 0x39479a40 | 453 | FUN_39479a40 | sale appraisal item count | stateful |  | 39897100,395720e0,394df370,397cce20 |
| U | 0x3947a040 | 708 | FUN_3947a040 | GetDecoActabilityPoint | stateful |  | 390b3850,39328630,39325ee0,397f22d0,398a8fe0,390b3a80,3930b100,3901c950,397a9330,390309f0,3901c950,396a6e00 +6 |
| U | 0x3947a310 | 446 | FUN_3947a310 | GetDecoActabilityLimitGroup | stateful |  | 390b3850,39328630,3963b390,3963b350,393251b0,3963b310,39001cc0,390309f0,390309f0,39009eb0,39058b50 |
| U | 0x3947a4d0 | 748 | FUN_3947a4d0 | GetHouseSaleInfo | stateful |  | 39460110,3963b390,39325660,398970e0,39009e40,398b562a,398b562a,398b562a,398b562a,39009e40,393b8e90,393b8e90 +5 |
| U | 0x394eaac0 | 1882 | FUN_394eaac0 | item-use task dispatcher: 5 house, 6 deco, 9 seed | stateful | 394eb270 | 3902db50,3902cef0,3950a1b0,3902b290,394e8590,3902db50,3902d670,396a6dc0,39330ba0,39696000,394d8b80,396a6e00 +40 |
| U | 0x394eb270 | 728 | FUN_394eb270 | item-task executor (caller of 394eaac0) | stateful | 391ff3e0,392005b0,39200880,39200b80,3944d590,394ebbe0 | 3915b270,3961e850,390457b0,39428370,39695dc0,39428370,39428370,394e1a90,39428370,39202fb0,39202db0,394e1950 +11 |

Total: 197 functions, 84374 bytes.

Manual sizes: 8 B state setters (`mov [ecx+0xc],imm; ret`), `FUN_3903b9d0` 33 B,
`FUN_3903ba00` 25 B, `FUN_39327de0` 2119 B, `FUN_393a8500` 64 B, `FUN_393349c0` 12 B, vtable stubs 16 B.
