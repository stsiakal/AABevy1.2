import csv, sys

LO, HI = 0x38ffdc0b, 0x3926de70
BASE = r'D:/aa-re/client12/out-x2game'


def load_funcs():
    rows = []
    with open(BASE + '/funcs.csv', encoding='utf-8', errors='replace') as f:
        for x in csv.DictReader(f):
            s = int(x['start'], 16)
            if LO <= s < HI:
                rows.append({'start': s, 'end': int(x['end'], 16),
                             'size': int(x['size']), 'name': x['name']})
    return rows


def main():
    rows = load_funcs()
    print('fns', len(rows), 'bytes', sum(r['size'] for r in rows),
          'KB', round(sum(r['size'] for r in rows) / 1024, 1))
    print('first', rows[0], 'last', rows[-1])
    named = [r for r in rows if not r['name'].startswith(('FUN_', 'thunk'))]
    print('named', len(named))
    for r in named[:40]:
        print(hex(r['start']), r['size'], r['name'])
    print('top sizes')
    for r in sorted(rows, key=lambda r: -r['size'])[:20]:
        print(hex(r['start']), r['size'], r['name'])


if __name__ == '__main__':
    main()
