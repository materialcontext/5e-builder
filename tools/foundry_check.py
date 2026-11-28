#!/usr/bin/env python3
"""Checks exported Foundry VTT actors against the dnd5e system's source.

Every enumerated value (skills, tools, languages, damage types, item and
spell types, units, ...) must be a key in the system's CONFIG
(module/config.mjs), every field name must occur in its data models, ids
must be 16 alphanumerics, and details links must point at embedded items.

    git clone --depth 1 https://github.com/foundryvtt/dnd5e
    python3 -I tools/foundry_check.py dnd5e out/*.foundry.json

With `--reference actor.json` (an actor exported from Foundry with
"Export Data"), it also lists field paths the export writes that the
reference never uses, per item type. That catches fields the system only
keeps for migration (e.g. `rarity` → `rarities`). Paths absent only
because the reference character doesn't have such a thing (no magic items,
no masteries) show up too, so read that list rather than fail on it.
"""
import json, re, sys, pathlib, collections
args = sys.argv[1:]
reference = None
if '--reference' in args:
    i = args.index('--reference'); reference = args[i + 1]; del args[i:i + 2]
root = pathlib.Path(args[0])
files = args[1:]
cfg = (root/'module/config.mjs').read_text()
def keys(name, depth=2):
    m = re.search(r'^DND5E\.%s = (?:new Proxy\()?\{' % re.escape(name), cfg, re.M)
    assert m, name
    i = m.end(); lvl = 1; out = []; buf = ''
    while lvl:
        c = cfg[i]
        if c == '{': lvl += 1
        elif c == '}': lvl -= 1
        i += 1; buf += c
    for k in re.findall(r'^ {%d}([A-Za-z0-9_]+):' % depth, buf, re.M): out.append(k)
    return set(out)
C = {n: keys(n) for n in ['abilities','skills','weaponTypes','weaponMasteries','armorTypes','actorSizes','damageTypes',
     'conditionTypes','spellSchools','itemProperties','consumableTypes','lootTypes','equipmentTypes','miscEquipmentTypes',
     'featureTypes','abilityActivationTypes','timePeriods','distanceUnits','rangeTypes','itemRarity','tools',
     'spellcasting','currencies','movementUnits','senses','movementTypes','armorProficiencies','weaponProficiencies']}
C['weaponIds'] = keys('weaponIds'); C['armorIds'] = keys('armorIds') | keys('shieldIds')
C['languages'] = set(re.findall(r'^ {6,8}([a-z]+):', cfg[cfg.index('DND5E.languages = {'):cfg.index('DND5E.communicationTypes')], re.M))
C['featSub'] = {}
ft = cfg[cfg.index('DND5E.featureTypes = {'):cfg.index('DND5E.itemProperties = {')]
for block in re.finditer(r'^  ([a-zA-Z]+): \{(.*?)^  \}', ft, re.M|re.S):
    C['featSub'][block.group(1)] = set(re.findall(r'^ {6}([a-zA-Z]+):', block.group(2), re.M))
prog = cfg[cfg.index('DND5E.spellcasting = {'):cfg.index('DND5E.spellPreparationStates')]
C['progression'] = set(re.findall(r'^ {6}([a-z]+): \{', prog, re.M)) | {'none'}
words = set()
for f in (root/'module').rglob('*.mjs'): words |= set(re.findall(r'[A-Za-z_][A-Za-z0-9_]*', f.read_text()))
errors = []
def chk(cond, msg):
    if not cond: errors.append(msg)
def inset(v, name, where, allow_empty=True):
    if allow_empty and v in ('', None): return
    chk(v in C[name], f'{where}: {v!r} not in {name}')
def walk_keys(o, path):
    if isinstance(o, dict):
        for k, v in o.items():
            # map keys (ids, levels, CONFIG keys) are data, not field names
            if not (re.fullmatch(r'[A-Za-z0-9]{16}', k) or k.isdigit() or path.endswith(('.abilities','.skills','.tools','.spells','.speeds','.ranges','.advancement','.activities','.value'))):
                chk(k in words or '5e-builder' in path or k == '5e-builder', f'{path}: unknown field {k!r}')
            walk_keys(v, f'{path}.{k}')
    elif isinstance(o, list):
        for x in o: walk_keys(x, path)
for p in files:
    a = json.load(open(p)); s = a['system']; n = a['name']
    walk_keys(a, n)
    for k in s['abilities']: inset(k, 'abilities', n)
    for k, v in s['skills'].items(): inset(k, 'skills', n); inset(v['ability'], 'abilities', n)
    for k in s['tools']: inset(k, 'tools', n)
    chk(set(s['skills']) == C['skills'], f'{n}: skills incomplete {C["skills"]-set(s["skills"])}')
    t = s['traits']; inset(t['size'], 'actorSizes', n)
    for k in ('dr','di','dv'):
        for v in t[k]['value']: inset(v, 'damageTypes', n)
    for v in t['ci']['value']: inset(v, 'conditionTypes', n)
    for v in t['languages']['value']: inset(v, 'languages', n)
    for v in t['armorProf']['value']: inset(v, 'armorProficiencies', n)
    for v in t['weaponProf']['value']: chk(v in C['weaponProficiencies'] or v in C['weaponIds'], f'{n}: weaponProf {v}')
    for v in t['weaponProf']['mastery']['value']: inset(v, 'weaponIds', n)
    for k in s['attributes']['movement']['speeds']: inset(k, 'movementTypes', n)
    for k in s['attributes']['senses']['ranges']: inset(k, 'senses', n)
    for k in s['currency']: inset(k, 'currencies', n)
    ids = [i['_id'] for i in a['items']]
    chk(len(ids) == len(set(ids)), f'{n}: duplicate ids')
    for d in (s['details']['race'], s['details']['background'], s['details']['originalClass']):
        chk(d == '' or d in ids, f'{n}: details link {d} dangling')
    for it in a['items']:
        w = f"{n}/{it['type']}:{it['name']}"; sy = it['system']
        chk(re.fullmatch(r'[A-Za-z0-9]{16}', it['_id']), f'{w}: bad id')
        chk(it['type'] in {'class','subclass','race','background','feat','spell','weapon','equipment','consumable','tool','loot','container'}, f'{w}: type')
        for prop in sy.get('properties', []): inset(prop, 'itemProperties', w)
        if 'rarity' in sy: inset(sy['rarity'], 'itemRarity', w)
        if it['type'] == 'weapon':
            inset(sy['type']['value'], 'weaponTypes', w, False); inset(sy['type']['baseItem'], 'weaponIds', w)
            inset(sy['mastery'], 'weaponMasteries', w)
            for dt in sy['damage']['base']['types']: inset(dt, 'damageTypes', w)
        if it['type'] == 'equipment':
            chk(sy['type']['value'] in C['armorTypes'] | C['equipmentTypes'] | C['miscEquipmentTypes'], f'{w}: equipment type')
            inset(sy['type']['baseItem'], 'armorIds', w)
        if it['type'] == 'consumable': inset(sy['type']['value'], 'consumableTypes', w)  # blank is valid (Oil, Torch)
        if it['type'] == 'loot': inset(sy['type']['value'], 'lootTypes', w, False)
        if it['type'] == 'tool': inset(sy['type']['baseItem'], 'tools', w)
        if it['type'] in ('class','subclass'): inset(sy['spellcasting']['progression'], 'progression', w, False); inset(sy['spellcasting']['ability'], 'abilities', w)
        if it['type'] == 'feat':
            tv = sy['type']['value']; chk(tv == '' or tv in C['featSub'], f'{w}: feat type {tv}')
            if sy['type']['subtype']: chk(sy['type']['subtype'] in C['featSub'].get(tv, set()), f'{w}: subtype {sy["type"]["subtype"]}')
        if it['type'] == 'spell':
            inset(sy['school'], 'spellSchools', w, False); inset(sy['method'], 'spellcasting', w, False)
            chk(sy['prepared'] in (0,1,2), f'{w}: prepared')
            inset(sy['ability'], 'abilities', w)
            inset(sy['activation']['type'], 'abilityActivationTypes', w, False)
            chk(sy['duration']['units'] in C['timePeriods'] | {'inst','perm','spec','turn','round','minute','hour','day','month','year'}, f'{w}: duration {sy["duration"]["units"]}')
            chk(sy['range']['units'] in C['movementUnits'] | C['rangeTypes'], f'{w}: range {sy["range"]["units"]}')
            for act in sy['activities'].values():
                for part in act.get('damage', {}).get('parts', []):
                    for dt in part['types']: inset(dt, 'damageTypes', w)
                for ab in act.get('save', {}).get('ability', []): inset(ab, 'abilities', w)
print('\n'.join(sorted(set(errors))[:80]) or 'OK', f'({len(set(errors))} problems)')

if reference:
    def paths(o, p, out):
        if isinstance(o, dict):
            for k, v in o.items():
                k2 = '*' if (re.fullmatch(r'[A-Za-z0-9]{16}', k) or k.isdigit() or p.endswith(('abilities','skills','tools','spells','speeds','ranges','activities','advancement','scale','flags','amount'))) else k
                if p.endswith('.flags'): continue
                paths(v, p + '.' + k2, out)
        elif isinstance(o, list):
            for x in o: paths(x, p + '[]', out)
        else: out.add(p)
    def collect(a):
        out = collections.defaultdict(set)
        paths(a['system'], 'actor', out['actor'])
        for it in a['items']: paths(it['system'], it['type'], out[it['type']])
        return out
    ref = collect(json.load(open(reference)))
    mine = collections.defaultdict(set)
    for p in files:
        for k, v in collect(json.load(open(p))).items(): mine[k] |= v
    print('\nField paths not used by the reference:')
    for t in sorted(mine):
        if t not in ref: print('  ', t, ': no item of this type in the reference'); continue
        extra = sorted(x for x in mine[t] if not any(r == x or r.startswith(x + '.') or r.startswith(x + '[') or x.startswith(r + '.') for r in ref[t]))
        if extra: print('  ', t, ':', ', '.join(extra))
sys.exit(1 if errors else 0)
