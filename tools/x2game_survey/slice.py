import csv, sys, collections, os

BASE = r'D:/aa-re/client12/out-x2game'


def main():
    lo = int(sys.argv[1], 16)
    hi = int(sys.argv[2], 16)
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 80
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
            if lo <= a < hi:
                xr[a].append(p[1])
    n = 0
    kb = 0
    rows = []
    with open(BASE + '/funcs.csv', encoding='utf-8', errors='replace') as f:
        for x in csv.DictReader(f):
            a = int(x['start'], 16)
            if lo <= a < hi:
                rows.append((a, int(x['size']), x['name']))
    rows.sort()
    printed = 0
    for a, sz, name in rows:
        n += 1
        kb += sz
        ss = xr.get(a, [])
        if (ss or n <= limit) and printed < limit:
            printed += 1
            print('%s %5d %s %s' % (hex(a), sz, name,
                                    ' || '.join(s.replace('\\n', ' ')[:110] for s in ss[:4])[:260]))
    print('TOTAL', n, 'fns', round(kb / 1024, 1), 'KB')


if __name__ == '__main__':
    main()
