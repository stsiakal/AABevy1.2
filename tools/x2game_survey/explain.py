import csv, sys, collections, re

BASE = r'D:/aa-re/client12/out-x2game'


def load_xrefs():
    xr = collections.defaultdict(list)
    with open(BASE + '/strings-xrefs.tsv', encoding='utf-8', errors='replace') as f:
        for line in f:
            p = line.rstrip('\n').split('\t')
            if len(p) < 3 or p[2] == '-':
                continue
            try:
                a = int(p[2], 16)
            except ValueError:
                continue
            xr[a].append(p[1])
    return xr


def main():
    addr = int(sys.argv[1], 16)
    xr = load_xrefs()
    sizes = {}
    with open(BASE + '/funcs.csv', encoding='utf-8', errors='replace') as f:
        for x in csv.DictReader(f):
            sizes[int(x['start'], 16)] = (int(x['size']), x['name'])
    out = collections.defaultdict(list)
    inn = collections.defaultdict(list)
    with open(BASE + '/calls.tsv', encoding='utf-8', errors='replace') as f:
        next(f)
        for line in f:
            p = line.rstrip('\n').split('\t')
            if len(p) < 4:
                continue
            try:
                a, b = int(p[0], 16), int(p[2], 16)
            except ValueError:
                continue
            out[a].append((b, p[3]))
            inn[b].append((a, p[1]))
    sz, name = sizes.get(addr, (0, '?'))
    print('FUNCTION', hex(addr), sz, name)
    print('STRINGS:', ' | '.join(s.replace('\\n', ' ')[:100] for s in xr.get(addr, [])[:8]))
    print('CALLEES:')
    for b, n in out.get(addr, [])[:25]:
        ss = xr.get(b, [])
        print('  %s %5d %s  %s' % (hex(b), sizes.get(b, (0, ''))[0], n,
                                   ' | '.join(s.replace('\\n', ' ')[:70] for s in ss[:2])[:150]))
    print('CALLERS:')
    for a, n in inn.get(addr, [])[:15]:
        ss = xr.get(a, [])
        print('  %s %5d %s  %s' % (hex(a), sizes.get(a, (0, ''))[0], n,
                                   ' | '.join(s.replace('\\n', ' ')[:60] for s in ss[:2])[:130]))
    text = open(BASE + '/x2game.dll.c', encoding='utf-8', errors='replace').read()
    i = text.find('// ==== FUN_%x @' % addr)
    if i < 0:
        i = text.find('// ==== thunk_FUN_%x @' % addr)
    if i >= 0:
        print('HEAD:')
        print('\n'.join(text[i:i + 1800].splitlines()[:30]))


if __name__ == '__main__':
    main()
