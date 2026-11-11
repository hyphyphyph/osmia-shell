#!/usr/bin/env python3
# Usage: gen_emoji.py emoji-test.txt annotations/en.xml data/emoji.tsv
# Sources: https://www.unicode.org/Public/emoji/latest/emoji-test.txt and
# https://github.com/unicode-org/cldr/blob/main/common/annotations/en.xml
# Builds data/emoji.tsv from emoji-test.txt and CLDR annotations/en.xml.
import re, sys, html
test, ann, out = sys.argv[1:4]
kw = {}
for m in re.finditer(r'<annotation cp="([^"]+)">([^<]*)</annotation>', open(ann, encoding='utf8').read()):
    kw[html.unescape(m[1]).replace('️', '')] = [k.strip() for k in html.unescape(m[2]).split('|')]
TONES = ['light', 'medium-light', 'medium', 'medium-dark', 'dark']
groups, rows, by_name, group = [], [], {}, None
for line in open(test, encoding='utf8'):
    if line.startswith('# group:'):
        group = line.split(':', 1)[1].strip()
        if group != 'Component': groups.append(group)
        continue
    m = re.match(r'^([0-9A-F ]+);\s*fully-qualified\s*# (\S+) E\d+\.\d+ (.*)$', line)
    if not m or group == 'Component': continue
    e, name = m[2], m[3].strip()
    tone = re.match(r'^(.*): (%s) skin tone$' % '|'.join(TONES), name)
    if tone and tone[1] in by_name:
        by_name[tone[1]][4][TONES.index(tone[2])] = e
        continue
    if 'skin tone' in name: continue
    r = [e, groups.index(group), name, kw.get(e.replace('️', ''), []), [None]*5]
    by_name[name] = r
    rows.append(r)
with open(out, 'w', encoding='utf8') as f:
    f.write('# osmia emoji list: emoji, group, name, keywords, five skin tones.\n')
    f.write('# Built from Unicode emoji-test.txt and CLDR annotations (Unicode License v3).\n')
    f.write('# groups\t' + '\t'.join(groups) + '\n')
    for e, g, name, k, t in rows:
        words = set(re.split(r'[\s:]+', name.lower()))
        k = ' '.join(x for x in k if x.lower() not in words)
        tones = ' '.join(t) if all(t) else ''
        f.write(f'{e}\t{g}\t{name}\t{k}\t{tones}\n')
print(len(rows), 'emoji', sum(1 for r in rows if all(r[4])), 'with tones', groups)
