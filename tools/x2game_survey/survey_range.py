import csv, re, os, sys, collections, glob

OUT = 'D:/aa-re/client12/out-x2game'
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
LO, HI = 0x397135d0, 0x3ac12242


# ---------------------------------------------------------------- inputs
def load_funcs():
    rows = []
    with open(os.path.join(OUT, 'funcs.csv'), encoding='utf-8') as fh:
        for r in csv.DictReader(fh):
            r['addr'] = int(r['start'], 16)
            r['size'] = int(r['size'])
            rows.append(r)
    return rows


def load_strings(addrs):
    out = collections.defaultdict(list)
    with open(os.path.join(OUT, 'strings-xrefs.tsv'), encoding='utf-8', errors='replace') as fh:
        next(fh)
        for line in fh:
            p = line.rstrip('\n').split('\t')
            if len(p) >= 4 and p[2] != '-':
                a = int(p[2], 16)
                if a in addrs:
                    out[a].append(p[1])
    return out


def load_calls():
    edges = collections.defaultdict(list)
    with open(os.path.join(OUT, 'calls.tsv'), encoding='utf-8', errors='replace') as fh:
        next(fh)
        for line in fh:
            p = line.rstrip('\n').split('\t')
            if len(p) >= 4 and not p[2].startswith('EXTERNAL'):
                try:
                    a, b = int(p[0], 16), int(p[2], 16)
                except ValueError:
                    continue
                edges[a].append(b)
    return edges


def load_docs():
    pat = re.compile(r'(?<![0-9a-f])(3[89ab][0-9a-f]{6})(?![0-9a-f])', re.I)
    hits = collections.defaultdict(set)
    for f in glob.glob(os.path.join(ROOT, 'docs/research/port-*/MAP.md')) + [os.path.join(ROOT, 'docs/research/port-program.md')]:
        rel = f.replace(os.sep, '/').split('docs/research/')[1]
        s = open(f, encoding='utf-8', errors='replace').read()
        for m in pat.findall(s):
            hits[int(m, 16)].add(rel)
    return hits


def load_ports():
    pat = re.compile(r'(?:0x|FUN_|@)(3[89ab][0-9a-f]{6})', re.I)
    hits = collections.defaultdict(set)
    for f in glob.glob(os.path.join(ROOT, 'crates/aa-client/src/port/**/*.rs'), recursive=True):
        rel = f.replace(os.sep, '/').split('crates/aa-client/src/port/')[1]
        s = open(f, encoding='utf-8', errors='replace').read()
        for m in pat.findall(s):
            hits[int(m, 16)].add(rel)
    return hits


def crate_words():
    words = set()
    for base, dirs, files in os.walk(os.path.join(ROOT, 'crates')):
        for fn in files:
            if fn.endswith('.rs'):
                try:
                    words.update(re.findall(r'[A-Za-z0-9_]+', open(os.path.join(base, fn), encoding='utf-8', errors='replace').read()))
                except OSError:
                    pass
    return words


# ---------------------------------------------------------------- areas
DOC_AREA = [
    ('port-formulas', 'Formulas/stats (mapped)'),
    ('port-combat', 'Combat/skills/cooldowns (mapped)'),
    ('port-engine-combat', 'Combat/skills/cooldowns (mapped)'),
    ('port-movement-extra', 'Movement/mounts/falling (mapped)'),
    ('port-mount-anim', 'Movement/mounts/falling (mapped)'),
    ('port-movement', 'Movement/mounts/falling (mapped)'),
    ('port-vehicles', 'Vehicles/ships (mapped)'),
    ('port-housing', 'Housing/farms (mapped)'),
    ('port-interaction', 'Interaction/doodad use (mapped)'),
    ('port-portals', 'Portals/teleport (mapped)'),
    ('port-flowgraph', 'Flowgraph/cutscenes (mapped)'),
    ('port-engine-char', 'Engine character/animation/audio (mapped)'),
    ('port-engine-anim-char', 'Engine character/animation/audio (mapped)'),
    ('port-engine-movie', 'Flowgraph/cutscenes (mapped)'),
    ('port-engine-ai', 'Engine AI (mapped)'),
    ('port-engine-core-rest', 'Engine core (mapped)'),
    ('port-engine-loop', 'Engine frame loop (mapped)'),
    ('port-program.md', 'Programme anchors/shared helpers (mapped)'),
]
PORT_AREA = [
    ('formulas/', 'Formulas/stats'),
    ('combat/', 'Combat/skills/cooldowns'),
    ('movement/', 'Movement/mounts/falling'),
    ('vehicles/', 'Vehicles/ships'),
    ('housing/', 'Housing/farms'),
    ('interaction/', 'Interaction/doodad use'),
    ('world/', 'World/streaming'),
    ('anim/', 'Animation'),
    ('camera/', 'Camera'),
]


def doc_area(docs):
    ds = sorted(docs)
    names = {d.split('/')[0] for d in ds}
    if len(names) > 2:
        return 'Shared/cross-system helpers (mapped)'
    for key, area in DOC_AREA:
        if any(d.startswith(key) for d in ds):
            return area
    return 'Other mapped client code'


def port_area(files):
    for f in sorted(files):
        for key, area in PORT_AREA:
            if f.startswith(key):
                return area + ' (ported)'
    return 'Other ported code'


# ---------------------------------------------------------------- loader domains
DOMAIN_RULES = [
    ('doodad_func', 'Doodad functions'),
    ('doodad', 'Doodads'),
    ('quest', 'Quests/plot'), ('plot', 'Quests/plot'), ('cinema', 'Quests/plot'),
    ('skill', 'Skills/effects/buffs'), ('effect', 'Skills/effects/buffs'), ('buff', 'Skills/effects/buffs'),
    ('aoe', 'Skills/effects/buffs'), ('damage', 'Skills/effects/buffs'),
    ('itemconv', 'Items/inventory'), ('item', 'Items/inventory'), ('mould', 'Items/inventory'),
    ('equip', 'Items/inventory'), ('enchant', 'Items/inventory'), ('gem', 'Items/inventory'),
    ('house', 'Housing/farms'), ('farm', 'Housing/farms'),
    ('vehicle', 'Vehicles/ships'), ('ship', 'Vehicles/ships'), ('slave_custom', 'Vehicles/ships'),
    ('portal', 'Portals/teleport'), ('teleport', 'Portals/teleport'), ('return_point', 'Portals/teleport'),
    ('zone', 'World/zones'), ('world', 'World/zones'), ('climate', 'World/zones'),
    ('sub_zone', 'World/zones'), ('district', 'World/zones'), ('fish', 'World/zones'),
    ('npc', 'Units/NPCs/models'), ('unit', 'Units/NPCs/models'), ('model', 'Units/NPCs/models'),
    ('character', 'Characters/customising'), ('custom', 'Characters/customising'),
    ('face', 'Characters/customising'), ('hair', 'Characters/customising'), ('race', 'Characters/customising'),
    ('anim', 'Animation/FX/audio'), ('fx_', 'Animation/FX/audio'), ('fx', 'Animation/FX/audio'),
    ('sound', 'Animation/FX/audio'), ('audio', 'Animation/FX/audio'),
    ('ui_', 'UI/text/chat'), ('text', 'UI/text/chat'), ('chat', 'UI/text/chat'),
    ('grammar', 'UI/text/chat'), ('font', 'UI/text/chat'), ('slash', 'UI/text/chat'),
    ('tag', 'UI/text/chat'), ('appellation', 'UI/text/chat'),
    ('craft', 'Crafting'), ('mould_pack', 'Crafting'),
    ('faction', 'Faction/expedition'), ('expedition', 'Faction/expedition'),
    ('achievement', 'Achievements/ranking'), ('rank', 'Achievements/ranking'),
    ('premium', 'Cash shop/premium'), ('cash', 'Cash shop/premium'), ('currency', 'Cash shop/premium'),
    ('mail', 'Mail/auction/trade'), ('auction', 'Mail/auction/trade'), ('trade', 'Mail/auction/trade'),
    ('siege', 'Siege/instances/battlefield'), ('indun', 'Siege/instances/battlefield'),
    ('battlefield', 'Siege/instances/battlefield'), ('tower_def', 'Siege/instances/battlefield'),
    ('conflict', 'Siege/instances/battlefield'),
    ('ai_', 'AI'), ('ai', 'AI'), ('schedule', 'Schedule/events'), ('game_rule', 'Schedule/events'),
    ('demo', 'Misc tables/settings'), ('prefab', 'Misc tables/settings'), ('constant', 'Misc tables/settings'),
    ('formula', 'Misc tables/settings'), ('system', 'Misc tables/settings'), ('world_var', 'Misc tables/settings'),
]


def domain_of(table):
    t = table.lower()
    for k, d in DOMAIN_RULES:
        if k in t:
            return d
    return 'Misc tables/settings'


# ---------------------------------------------------------------- classification
def main():
    allrows = load_funcs()
    rows = [r for r in allrows if LO <= r['addr'] < HI]
    by_addr = {r['addr']: r for r in rows}
    strings = load_strings(set(by_addr))
    calls = load_calls()
    docs = load_docs()
    ports = load_ports()
    words = crate_words()

    seed = set(a for a in calls.get(0x3960b820, ()) if a in by_addr)
    seed_tables = {}
    for a in seed:
        tab = None
        names = []
        for s in strings.get(a, ()):
            m = re.search(r'\bFROM\s+([A-Za-z0-9_]+)', s)
            if m and tab is None:
                tab = m.group(1)
            m = re.search(r'function=Load([A-Za-z0-9_]+)Descs', s)
            if m:
                names.append(m.group(1))
        seed_tables[a] = (tab, names)

    def base(f):
        a = f['addr']
        n = f['name']
        if n.startswith('Unwind@'):
            return 'MSVC C++ unwind records (compiler)', 'OUT', 'unwind metadata, not code; Rust panic/unwind replaces it'
        if n.startswith('Catch') or n.startswith('__ArrayUnwind') or 'eh_vector' in n:
            return 'MSVC exception handlers (compiler)', 'OUT', 'compiler-generated catch/handler glue'
        if n.startswith('thunk_'):
            return 'Compiler thunks / import jump stubs', 'OUT', 'thunk, no behaviour of its own'
        if a < 0x397a6ba0:
            return ('SQLite descriptor framework + STL templates (client DB)', 'OUT',
                    'generated table containers/validators and STL sort/map/heap code; replaced by rusqlite + Rust std')
        if a < 0x397ae860:
            return ('Client doodad/housing/vehicle/portal gameplay + property codec', 'COVERED',
                    'aa-client/src/port + hud and aa-net implement the same systems (not proven equal)')
        if a < 0x397d5000:
            return ('X2 packet/archive codec (all packet families)', 'COVERED',
                    'aa-net implements the 1.2 protocol; not proven equal per packet')
        if a < 0x39838000:
            return ('SQLite descriptor framework + STL templates (client DB)', 'OUT',
                    'generated table containers/validators and STL sort/map/heap code; replaced by rusqlite + Rust std')
        if a < 0x3983f000:
            return ('Game-object state / housing-farm property codec', 'COVERED',
                    'aa-net + aa-client hud/housing/port implement object state (not proven equal)')
        if a < 0x39897000:
            return ('SQLite descriptor framework + STL templates (client DB)', 'OUT',
                    'generated table containers/validators and STL sort/map/heap code; replaced by rusqlite + Rust std')
        if a < 0x39898000:
            return ('Client housing/interaction logic', 'COVERED',
                    'aa-client/src/port/housing + hud and port-housing MAP (not proven equal)')
        if a < 0x39899000:
            return ('Client housing/interaction/quest-context logic', 'COVERED',
                    'aa-client hud + aa-net quest/interaction paths (not proven equal)')
        if a < 0x3989e000:
            return ('Client combat/skill cooldown and effect logic', 'COVERED',
                    'aa-client/src/port/combat + hud combat/buffs (not proven equal)')
        if a < 0x398aa000:
            return ('Client combat damage/skill targeting logic', 'COVERED',
                    'aa-client/src/port/combat + hud combat (not proven equal)')
        if a < 0x398b5000:
            return ('Client data managers: ability/faction/mail/auction/zone', 'COVERED',
                    'aa-client hud/{achievement,auction,faction,mail,...} + aa-net (not proven equal)')
        if a < 0x398b6000:
            return ('Formula/stat engine and CRT startup', 'COVERED',
                    'aa-client/src/port/formulas (port-formulas MAP); not proven equal')
        if a < 0x398c8000:
            return ('HSUpdate telemetry/anti-cheat + updater', 'OUT',
                    'hsupdate.env/HSUpChk.log, encrypted payloads, multipart log upload; not game behaviour')
        if a < 0x398cba00:
            return ('QOS network-quality SDK (Tencent)', 'OUT',
                    'QOSSSPkg/udp://, tqos_* strings; telemetry/QoS only')
        if a < 0x398f9000:
            return ('FreeType font engine (statically linked)', 'COVERED',
                    'aa-ui/Bevy text stack parses fonts; not proven equal')
        if a < 0x398ff000:
            return ('zlib inflate/deflate (statically linked)', 'COVERED',
                    'aa-pak/flate2 decompress the same streams; not proven equal')
        if a < 0x39925de0:
            return ('libcurl 7.21.2 (HTTP/FTP/IMAP/...)', 'OUT',
                    'used by HSUpdate/updater, not gameplay; replaced by no client crate')
        if a < 0x3992a000:
            return ('TDR anti-cheat protocol + network helpers', 'OUT',
                    'TDR_* metadata/network validation and host parsing; anti-cheat only')
        if a < 0x399a6800:
            return ('C++ static initialisation of game-data globals', 'OUT',
                    'global ctors/_atexit registration, no behaviour')
        if f['size'] <= 12:
            return ('Compiler thunks / import jump stubs', 'OUT', 'tiny cold stub/getter, no own behaviour')
        return ('Unclassified cold code fragments (0x3a66-0x3ac1 islands)', 'UNKNOWN',
                'cold fragment outside any map; calls hot game helpers, purpose not established')

    per = {}
    for f in rows:
        a = f['addr']
        if a in ports:
            area = port_area(ports[a])
            status = 'PORTED'
            ev = 'port: ' + ', '.join('crates/aa-client/src/port/' + x for x in sorted(ports[a]))
        elif a in docs:
            area = doc_area(docs[a])
            status = 'MAPPED'
            ev = 'MAP: ' + ', '.join('docs/research/' + x for x in sorted(docs[a]))
        else:
            area, status, ev = base(f)
            if a in seed_tables and not f['name'].startswith('Unwind@'):
                tab, names = seed_tables[a]
                if tab:
                    d = domain_of(tab)
                    cov = tab in words
                    area = 'DB descriptors: ' + d
                    status = 'COVERED' if cov else 'TO-PORT'
                    ev = 'table `%s` %s' % (tab, 'read by a crate (not proven equal)' if cov else 'not read by any crate; no MAP')
                else:
                    area = 'DB descriptor load'
                    status = 'COVERED'
                    ev = 'master-loader callee without a parsed table name'
        per[a] = (area, status, ev, f)

    # per-status totals
    tot = collections.defaultdict(lambda: [0, 0])
    for a, (area, status, ev, f) in per.items():
        tot[status][0] += 1
        tot[status][1] += f['size']
    print('TOTALS')
    for s in ['PORTED', 'MAPPED', 'COVERED', 'TO-PORT', 'OUT', 'UNKNOWN']:
        print('%-8s %6d %10d B %8.1f KB' % (s, tot[s][0], tot[s][1], tot[s][1] / 1024))
    print('sum', sum(v[0] for v in tot.values()), sum(v[1] for v in tot.values()))

    # groups
    grp = collections.defaultdict(lambda: [0, 0, [], set(), set()])
    for a, (area, status, ev, f) in per.items():
        g = grp[(area, status)]
        g[0] += 1
        g[1] += f['size']
        if len(g[2]) < 3:
            g[2].append((a, f['size'], f['name']))
        if ev.startswith('MAP:'):
            g[3].add(ev)
        if ev.startswith('port:'):
            g[4].add(ev)
    print('\nGROUPS')
    for (area, status), g in sorted(grp.items(), key=lambda kv: -kv[1][1]):
        print('%-55s %-8s %5d %7.1f KB  %s' % (area[:55], status, g[0], g[1] / 1024, '; '.join(sorted(g[3]))[:70] or '; '.join(sorted(g[4]))[:70]))

    # dump classification for inspection (git-ignored, under data/)
    outdir = os.path.join(ROOT, 'data', 'extract', 'x2game-survey')
    os.makedirs(outdir, exist_ok=True)
    with open(os.path.join(outdir, 'r4-classification.tsv'), 'w', encoding='utf-8') as fh:
        fh.write('addr\tsize\tname\tstatus\tarea\tevidence\n')
        for a in sorted(per):
            area, status, ev, f = per[a]
            fh.write('%08x\t%d\t%s\t%s\t%s\t%s\n' % (a, f['size'], f['name'], status, area, ev))

    if '--check' in sys.argv:
        # Re-read the written artifact and prove it covers the funcs.csv range exactly.
        with open(os.path.join(outdir, 'r4-classification.tsv'), encoding='utf-8') as fh:
            written = list(csv.DictReader(fh, delimiter='\t'))
        expect_fns = len(rows)
        expect_bytes = sum(r['size'] for r in rows)
        got_fns = len(written)
        got_bytes = sum(int(r['size']) for r in written)
        statuses = collections.Counter(r['status'] for r in written)
        assert (got_fns, got_bytes) == (expect_fns, expect_bytes), (got_fns, got_bytes, expect_fns, expect_bytes)
        assert set(statuses) <= {'PORTED', 'MAPPED', 'COVERED', 'TO-PORT', 'OUT', 'UNKNOWN'}, statuses
        assert sum(statuses.values()) == got_fns, statuses
        assert len({r['addr'] for r in written}) == got_fns, 'duplicate address in classification'
        print('PASS: %d functions, %d bytes; status sums %s' % (got_fns, got_bytes, dict(sorted(statuses.items()))))


if __name__ == '__main__':
    main()
