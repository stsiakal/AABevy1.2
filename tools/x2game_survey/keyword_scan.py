import csv, re, sys, collections, os

LO, HI = 0x38ffdc0b, 0x3926de70
BASE = r'D:/aa-re/client12/out-x2game'
OUT = os.path.join(os.environ.get('TEMP', r'C:/Users/stsiakal/AppData/Local/Temp'), 'opencode', 'x2game')
os.makedirs(OUT, exist_ok=True)


def load_funcs():
    rows = []
    with open(BASE + '/funcs.csv', encoding='utf-8', errors='replace') as f:
        for x in csv.DictReader(f):
            s = int(x['start'], 16)
            if LO <= s < HI:
                rows.append({'start': s, 'end': int(x['end'], 16),
                             'size': int(x['size']), 'name': x['name']})
    rows.sort(key=lambda r: r['start'])
    return rows


def load_xrefs():
    xr = collections.defaultdict(list)
    with open(BASE + '/strings-xrefs.tsv', encoding='utf-8', errors='replace') as f:
        for line in f:
            parts = line.rstrip('\n').split('\t')
            if len(parts) < 3:
                continue
            fa = parts[2]
            if fa == '-':
                continue
            try:
                addr = int(fa, 16)
            except ValueError:
                continue
            if LO <= addr < HI:
                xr[addr].append((int(parts[0], 16), parts[1]))
    return xr


def load_calls():
    out = collections.defaultdict(set)
    inn = collections.defaultdict(set)
    with open(BASE + '/calls.tsv', encoding='utf-8', errors='replace') as f:
        next(f)
        for line in f:
            p = line.rstrip('\n').split('\t')
            if len(p) < 4:
                continue
            try:
                a = int(p[0], 16)
                b = int(p[2], 16)
            except ValueError:
                continue
            if LO <= a < HI:
                out[a].add(b)
            if LO <= b < HI:
                inn[b].add(a)
    return out, inn


def main():
    funcs = load_funcs()
    by_addr = {r['start']: r for r in funcs}
    xr = load_xrefs()
    calls_out, calls_in = load_calls()

    # per-function string digest
    with open(os.path.join(OUT, 'funcs.tsv'), 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, delimiter='\t')
        w.writerow(['start', 'size', 'name', 'n_callers', 'n_callees', 'strings'])
        for r in funcs:
            ss = []
            seen = set()
            for _, s in sorted(xr.get(r['start'], []), key=lambda t: t[0]):
                s = s.replace('\\n', ' ').replace('\\t', ' ')
                if s in seen:
                    continue
                seen.add(s)
                ss.append(s)
            w.writerow([hex(r['start']), r['size'], r['name'],
                        len(calls_in.get(r['start'], ())), len(calls_out.get(r['start'], ())),
                        ' || '.join(ss[:12])])

    # keyword aggregation
    kw = ['X2Unit:', 'X2Player:', 'X2Item:', 'X2Skill', 'Craft', 'Auction', 'Quest', 'Skill',
          'Item', 'Chat', 'Guild', 'House', 'Vehicle', 'Mount', 'Trade', 'Mail', 'Party',
          'Raid', 'Zone', 'Doodad', 'Npc', 'AI', 'Physics', 'Anim', 'Sound', 'Lua',
          'GameClient', 'CS', 'SC', 'packet', 'Unit', 'Player', 'World', 'Camera', 'Input',
          'Effect', 'Model', 'Mesh', 'Texture', 'Shader', 'Render', 'Terrain', 'Water',
          'Weather', 'Time', 'Login', 'Character', 'Account', 'Server', 'Client', 'DB', 'SQL']
    print('=== keyword hits (functions / KB / address span) ===')
    for k in kw:
        hits = set()
        for addr, lst in xr.items():
            if any(k.lower() in s.lower() for _, s in lst):
                if addr in by_addr:
                    hits.add(addr)
        if hits:
            kb = sum(by_addr[a]['size'] for a in hits) / 1024
            mn, mx = min(hits), max(hits)
            print('%-12s %5d fns %7.1f KB  %s..%s' % (k, len(hits), kb, hex(mn), hex(mx)))


if __name__ == '__main__':
    main()
