import csv, sys, collections

BASE = r'D:/aa-re/client12/out-x2game'


def main():
    lo = int(sys.argv[1], 16)
    hi = int(sys.argv[2], 16)
    seen = collections.Counter()
    first = {}
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
                s = p[1].replace('\\n', ' ').replace('\\t', ' ')
                if s.startswith('invalid map/set') or s.startswith('list<T>') or s.startswith('deque<T>'):
                    continue
                seen[s] += 1
                first.setdefault(s, a)
    for s, c in seen.most_common(200):
        print('%5d %s  %s' % (c, hex(first[s]), s[:160]))


if __name__ == '__main__':
    main()
