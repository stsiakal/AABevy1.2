import csv, re, os, collections, sys

LO, HI = 0x38ffdc0b, 0x3926de70
ROOT = r'C:/AAEMU_DISK/aaemu-client/wt/map3'
BASE = r'D:/aa-re/client12/out-x2game'


def load_funcs():
    rows = []
    with open(BASE + '/funcs.csv', encoding='utf-8', errors='replace') as f:
        for x in csv.DictReader(f):
            s = int(x['start'], 16)
            if LO <= s < HI:
                rows.append((s, int(x['size']), x['name']))
    rows.sort()
    return rows


HEX = re.compile(r'(?<![0-9A-Za-z_])(?:0x)?([0-9a-fA-F]{8})(?![0-9A-Za-z_])')
RANGE = re.compile(r'(?:0x)?([0-9a-fA-F]{8})\s*(?:-|–|—|\.\.|to)\s*(?:0x)?([0-9a-fA-F]{8})')


def cites():
    out = collections.defaultdict(list)
    for root, dirs, files in os.walk(os.path.join(ROOT, 'docs', 'research')):
        if not os.path.basename(root).startswith('port-'):
            continue
        for fn in files:
            if not fn.endswith('.md'):
                continue
            p = os.path.join(root, fn)
            rel = os.path.relpath(p, os.path.join(ROOT, 'docs', 'research')).replace('\\', '/')
            with open(p, encoding='utf-8', errors='replace') as f:
                text = f.read()
            for m in HEX.finditer(text):
                a = int(m.group(1), 16)
                if LO <= a < HI:
                    out[a].append(rel)
            for m in RANGE.finditer(text):
                a, b = int(m.group(1), 16), int(m.group(2), 16)
                if a > b:
                    a, b = b, a
                if LO <= b and a < HI:
                    out[('range', a, b)].append(rel)
    return out


def port_cites():
    out = collections.defaultdict(list)
    proot = os.path.join(ROOT, 'crates', 'aa-client', 'src', 'port')
    for root, dirs, files in os.walk(proot):
        for fn in files:
            if not fn.endswith('.rs'):
                continue
            p = os.path.join(root, fn)
            rel = os.path.relpath(p, proot).replace('\\', '/')
            with open(p, encoding='utf-8', errors='replace') as f:
                text = f.read()
            for m in HEX.finditer(text):
                a = int(m.group(1), 16)
                if LO <= a < HI:
                    out[a].append(rel)
    return out


def main():
    funcs = load_funcs()
    c = cites()
    pc = port_cites()
    ranged = [k for k in c if isinstance(k, tuple)]
    direct = {k: v for k, v in c.items() if not isinstance(k, tuple)}
    hit_direct = set(a for a, _, _ in funcs) & set(direct)
    hit_port = set(a for a, _, _ in funcs) & set(pc)
    hit_range = set()
    sample_ranges = []
    for (_, a, b) in ranged:
        hit = [x for x, _, _ in funcs if a <= x <= b]
        hit_range.update(hit)
        if len(hit) > 300:
            sample_ranges.append((a, b, len(hit), c[('range', a, b)][0]))
    print('direct cited fns', len(hit_direct), 'ported', len(hit_port))
    print('ranges', len(ranged), 'fns covered by ranges', len(hit_range))
    print('union', len(hit_direct | hit_port | hit_range))
    print('big ranges:', [(hex(a), hex(b), n, f) for a, b, n, f in sorted(sample_ranges, key=lambda t: -t[2])[:15]])
    with open(os.path.join(os.environ.get('TEMP'), 'opencode', 'x2game', 'cites.tsv'), 'w', encoding='utf-8') as f:
        f.write('addr\ttotal_size\tname\tmap_cites\tport_cites\n')
        for a, sz, n in funcs:
            f.write('%s\t%d\t%s\t%s\t%s\n' % (hex(a), sz, n,
                                             ','.join(sorted(set(direct.get(a, [])))),
                                             ','.join(sorted(set(pc.get(a, []))))))


if __name__ == '__main__':
    main()
