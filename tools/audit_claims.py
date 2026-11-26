"""Claims-vs-grants checker: compares what each feat/feature/background says in its rules text with what the compendium grants.
Usage: python3 -I tools/audit_claims.py data/5etools/data crates/web/public/compendium.json /tmp/audit.json  (RELAX=1 also checks conditional sentences)"""
import json, re, sys, glob, collections

DATA, COMP, OUT = sys.argv[1:4]
comp = json.load(open(COMP))
SKILLS = ["acrobatics","animal handling","arcana","athletics","deception","history","insight","intimidation","investigation","medicine","nature","perception","performance","persuasion","religion","sleight of hand","stealth","survival"]
ABIL = {"strength":"str","dexterity":"dex","constitution":"con","intelligence":"int","wisdom":"wis","charisma":"cha"}
tool_names = set(i["name"].lower() for i in comp["items"] if i["kind"] in ("Artisan's Tools","Tools","Gaming Set","Musical Instrument"))

def text_of(v):
    if isinstance(v, str): return v
    if isinstance(v, list): return "\n".join(text_of(x) for x in v)
    if isinstance(v, dict):
        if v.get("type") in ("refClassFeature","refSubclassFeature","refOptionalfeature"): return ""
        return "\n".join(text_of(x) for k, x in v.items() if k in ("entries","entry","items","name","rows"))
    return ""

def sentences(t):
    return re.split(r"(?<=[.!?])\s+|\n", t)

import os
RELAX = os.environ.get("RELAX") == "1"
CONDITIONAL_RE = re.compile(r"\b(while|when you|whenever|until|if you|for the duration|for 1 (minute|hour)|as a (bonus )?action|as a reaction|you can use|expend|spend|for the next|this turn|each time|during)\b", re.I)

class _C:
    def search(self, x):
        return None if RELAX else CONDITIONAL_RE.search(x)
CONDITIONAL = _C()

def claims(t):
    c = collections.defaultdict(set)
    for s in sentences(t):
        low = s.lower()
        plain = re.sub(r"\{@\w+ ([^}|]*)[^}]*\}", r"\1", s).lower()
        if "proficien" in low and not re.search(r"already (have|has) proficiency|if you('re| are) already proficient|double|twice your proficiency|add your proficiency bonus|proficiency bonus to|equal to your proficiency bonus|proficiency bonus is", low):
            for m in re.findall(r"\{@skill ([^}|]+)", s): c["skill"].add(m.lower())
            for m in re.findall(r"\{@item ([^}|]+)", s):
                if m.lower() in tool_names: c["tool"].add(m.lower())
            for a in ("heavy armor","medium armor","light armor","shields"):
                if a in plain: c["armor"].add(a)
            for w in ("martial weapons","simple weapons"):
                if w in plain: c["weapon"].add(w)
            for name, ab in ABIL.items():
                if re.search(name + r" saving throws?", plain) and "proficiency in" in plain: c["save"].add(ab)
            if re.search(r"(one|two|three|a) (skill|tool)s? of your choice|skills? of your choice", plain): c["choice"].add("skill/tool choice")
        if re.search(r"\bexpertise\b|proficiency bonus is doubled|double your proficiency bonus", low) and re.search(r"choose|in (the )?\{@skill|following skills|of your skill", low):
            c["expertise"].add("expertise")
        if re.search(r"(learn|speak, read, and write|you know) (one|two|three|an?) (additional |extra |other )?languages?|languages? of your choice", plain):
            c["language"].add("language")
        m = re.search(r"increase (?:your |one )?(\w+)(?:,| or| and)? .*?score[s]? by (\d)", plain)
        if m and "ability score improvement" not in plain:
            c["asi"].add(m.group(0)[:60])
        for m in re.findall(r"\{@spell ([^}|]+)", s):
            if re.search(r"\byou (learn|know|always have|have the|gain)\b|always prepared|you can cast (it|this spell|the|\{@spell)[^.]*(without (expending )?a spell slot|once)", low):
                c["spell"].add(m.lower())
        if re.search(r"you (have|gain) resistance to|you are resistant to", low) and not CONDITIONAL.search(low):
            for m in re.findall(r"(acid|bludgeoning|cold|fire|force|lightning|necrotic|piercing|poison|psychic|radiant|slashing|thunder) damage", plain): c["resist"].add(m)
        if re.search(r"darkvision (out to|with a range of|of) (\d+)", plain) and not CONDITIONAL.search(low): c["sense"].add("darkvision")
        for sense in ("blindsight","truesight","tremorsense"):
            if re.search(sense + r" (out to|with a range of|of) \d+|you have " + sense, plain) and not CONDITIONAL.search(low): c["sense"].add(sense)
        if re.search(r"hit point maximum increases by", plain): c["hp"].add("hp")
        if re.search(r"(walking )?speed increases by \d+|you (have|gain) a (swim|fly|climb|burrow)(ming|ing)? speed", plain) and not CONDITIONAL.search(low): c["speed"].add("speed")
        if re.search(r"you gain (the|an?) \{@feat|origin feat of your choice|feat of your choice", low): c["feat"].add("feat")
    return c

def grants_summary(gs, out=None):
    out = out if out is not None else collections.defaultdict(set)
    for g in gs:
        if not isinstance(g, dict):
            if g == "AnySkill": out["choice"].add("x")
            continue
        (k, v), = g.items()
        if k == "Prof":
            (pk, pv), = v.items()
            m = {"Skill":"skill","Tool":"tool","Armor":"armor","Weapon":"weapon","Language":"language","Save":"save"}[pk]
            out[m].add(str(pv).lower()); 
        elif k == "Expertise": out["expertise"].add(v)
        elif k == "Ability": out["asi"].add(v["ability"])
        elif k == "Feat": out["feat"].add(v)
        elif k == "Spells":
            for sg in v["spells"]:
                p = sg["pick"]
                if "Fixed" in p: out["spell"].add(p["Fixed"].split("|")[0])
                else: out["spell"].add("*choice*")
            if v["ability"] and isinstance(v["ability"], dict) and "Choose" in v["ability"]: pass
        elif k == "Resist": out["resist"].add(v)
        elif k in ("SpeedBonus","SpeedSet"): out["speed"].add(v["mode"])
        elif k in ("HpPerClassLevel","HpBonus"): out["hp"].add("hp")
        elif k == "ToolExpertise": out["expertise"].add(v)
        elif k in ("Darkvision","DarkvisionBonus"): out["sense"].add("darkvision")
        elif k == "Sense": out["sense"].add(v["sense"])
        elif k == "HpPerLevel": out["hp"].add("hp")
        elif k == "Feature": pass
        elif k == "AtLevel": grants_summary(v["grants"], out)
        elif k == "Choose":
            o = v["options"]
            on = o if isinstance(o, str) else next(iter(o))
            kind = {"Skills":"skill","AnySkill":"skill","Tools":"tool","AnyTool":"tool","AnyArtisanTool":"tool","AnyInstrument":"tool","AnyGamingSet":"tool",
                    "Languages":"language","AnyLanguage":"language","AnyStandardLanguage":"language","AnyExoticLanguage":"language",
                    "SkillExpertise":"expertise","ExpertiseFrom":"expertise","Abilities":"asi","Feats":"feat","Spells":"spell","Saves":"save",
                    "Resist":"resist","Weapons":"weapon","WeaponMastery":"mastery","OptionalFeatures":"optfeature","Mixed":"choice"}.get(on)
            if kind:
                out[kind].add("*choice*")
                if kind in ("skill","tool","choice"): out["choice"].add("x")
            if on == "Mixed": out["skill"].add("*choice*"); out["tool"].add("*choice*"); out["language"].add("*choice*")
            if on == "Bundles":
                for b in o["Bundles"]: grants_summary(b["grants"], out)
    return out

def missing(cl, gr):
    issues = []
    for kind, vals in cl.items():
        have = gr.get(kind, set())
        if kind in ("skill","tool"):
            if "*choice*" in have or "x" in gr.get("choice", set()): continue
            miss = [v for v in vals if v not in have]
            if miss: issues.append(f"{kind}: {sorted(miss)}")
        elif kind == "spell":
            if "*choice*" in have: miss = []
            else: miss = [v for v in vals if v not in have]
            if miss: issues.append(f"spell: {sorted(miss)}")
        elif kind == "armor":
            miss = [v for v in vals if v.split()[0].rstrip('s') not in " ".join(have) ]
            if miss: issues.append(f"armor: {sorted(miss)}")
        elif kind == "weapon":
            miss = [v for v in vals if v.split()[0] not in " ".join(have)]
            if miss: issues.append(f"weapon: {sorted(miss)}")
        elif kind == "save":
            if "*choice*" in have: continue
            miss = [v for v in vals if v not in have]
            if miss: issues.append(f"save: {sorted(miss)}")
        elif kind == "resist":
            if "*choice*" in have: continue
            miss = [v for v in vals if v not in have]
            if miss: issues.append(f"resist: {sorted(miss)}")
        elif kind == "sense":
            miss = [v for v in vals if v not in have]
            if miss: issues.append(f"sense: {sorted(miss)}")
        else:
            if not have: issues.append(f"{kind}: {sorted(vals)[:2]}")
    return issues

results = []
def add(kind, key, name, text, grants, extra=None):
    cl = claims(text)
    gr = grants_summary(grants)
    iss = missing(cl, gr)
    if "{@" in json.dumps(extra or ""): iss.append("unstripped tag")
    results.append({"kind": kind, "key": key, "name": name, "issues": iss})

# raw text lookups
raw_feats = {(f["name"].lower(), f["source"].lower()): f for f in json.load(open(f"{DATA}/feats.json"))["feat"]}
raw_bgs = {(b["name"].lower(), b["source"].lower()): b for b in json.load(open(f"{DATA}/backgrounds.json"))["background"]}
cf, scf = {}, {}
for fn in glob.glob(f"{DATA}/class/class-*.json"):
    d = json.load(open(fn))
    for f in d.get("classFeature", []): cf[(f["name"].lower(), f["className"].lower(), f["classSource"].lower(), f["level"])] = f
    for f in d.get("subclassFeature", []): scf.setdefault((f["name"].lower(), f["className"].lower(), f["subclassShortName"].lower(), f["level"]), f)

def body_text(entries):
    def t(e):
        if isinstance(e, dict):
            (k, v), = e.items()
            if k == "P": return v
            if k == "Section": return (v["name"] or "") + ". " + "\n".join(t(x) for x in v["body"])
            if k == "List": return "\n".join(t(x) for x in v)
            if k == "Table": return ""
        return ""
    return "\n".join(t(e) for e in entries)

for f in comp["feats"]:
    raw = raw_feats.get(tuple(f["key"].split("|")))
    text = text_of(raw.get("entries", [])) if raw and raw.get("entries") else body_text(f["body"])
    add("feat", f["key"], f["name"], text, f["grants"], f["body"])
for b in comp["backgrounds"]:
    raw = raw_bgs.get(tuple(b["key"].split("|")))
    text = text_of(raw.get("entries", [])) if raw and raw.get("entries") else ""
    # 2024 backgrounds list their benefits as items; feature text for legacy
    add("background", b["key"], b["name"], text, b["grants"], b["features"])
for c in comp["classes"]:
    cname, csrc = c["key"].split("|")
    for f in c["features"]:
        raw = cf.get((f["name"].lower(), cname, csrc, f["level"]))
        text = text_of(raw["entries"]) if raw and raw.get("entries") else body_text(f["body"])
        add("classFeature", f'{c["key"]} L{f["level"]}', f["name"], text, f["grants"] + c["grants"], f["body"])
for s in comp["subclasses"]:
    short = s["short_name"].lower(); cname = s["class"].split("|")[0]
    for f in s["features"]:
        raw = scf.get((f["name"].lower(), cname, short, f["level"]))
        text = text_of(raw["entries"]) if raw and raw.get("entries") else body_text(f["body"])
        cls = next((c for c in comp["classes"] if c["key"] == s["class"]), {"grants": []})
        add("subclassFeature", f'{s["key"]} L{f["level"]}', f["name"], text, f["grants"] + s["grants"] + cls["grants"], f["body"])

json.dump(results, open(OUT, "w"), indent=1)
by = collections.Counter(r["kind"] for r in results)
flag = collections.Counter(r["kind"] for r in results if r["issues"])
print("records:", dict(by)); print("flagged:", dict(flag))
cat = collections.Counter(i.split(":")[0] for r in results for i in r["issues"])
print("issue kinds:", dict(cat))
