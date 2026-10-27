import csv, re, os, collections, math

LO, HI = 0x38ffdc0b, 0x3926de70
BASE = r'D:/aa-re/client12/out-x2game'
OUT = os.path.join(os.environ.get('TEMP', r'C:/Users/stsiakal/AppData/Local/Temp'), 'opencode', 'x2game')
BIN = 0x4000

funcs = {}
with open(BASE + '/funcs.csv', encoding='utf-8', errors='replace') as f:
    for x in csv.DictReader(f):
        s = int(x['start'], 16)
        if LO <= s < HI:
            funcs[s] = (int(x['size']), x['name'])

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
        if a in funcs:
            xr[a].append(p[1])

STOP = re.compile(r'%[sdifxXpuc]|^[A-Za-z_][A-Za-z0-9_]*$')
LABEL = re.compile(r'::|\.cpp|\.h\b|/|<[A-Z]|^X2|^C[A-Z][a-zA-Z]+$|^SC|^CS|^\(')


def score(s):
    v = 0
    if '::' in s: v += 5
    if '.cpp' in s or '.h' in s: v += 3
    if '/' in s or '\\' in s: v += 2
    if '<' in s: v += 2
    if 'X2' in s: v += 4
    if len(s) > 25: v += 1
    if STOP.match(s): v -= 5
    return v


bins = collections.defaultdict(list)
for a, (sz, n) in funcs.items():
    bins[a // BIN].append(a)

lines = []
for b in sorted(bins):
    addrs = sorted(bins[b])
    total = sum(funcs[a][0] for a in addrs)
    scored = []
    seen = set()
    for a in addrs:
        for s in xr.get(a, []):
            s = s.replace('\\n', ' ').replace('\\t', ' ')
            if s in seen or len(s) > 120:
                continue
            seen.add(s)
            sc = score(s)
            if sc > 0:
                scored.append((sc, s))
    top = []
    seen2 = set()
    for sc, s in sorted(scored, key=lambda t: (-t[0], t[1])):
        k = s.lower()
        if k in seen2:
            continue
        seen2.add(k)
        top.append(s)
        if len(top) >= 7:
            break
    lines.append('%s  %3d fns %7.1f KB  %s' % (hex(b * BIN), len(addrs), total / 1024, ' | '.join(top)))

with open(os.path.join(OUT, 'bins.txt'), 'w', encoding='utf-8') as f:
    f.write('\n'.join(lines) + '\n')
print('\n'.join(lines[:35]))
