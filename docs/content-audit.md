# Content audit: feats, class features, subclass features, backgrounds

Data: 5etools v2.41.3. Scope: every feat (326), background (175), class feature
(890 including reskinned classes) and subclass feature (2,324 including 2014
subclasses attached to 2024 classes) — 3,715 records.

## Method

1. **Claims vs. grants** (`tools/audit_claims.py`). For each record, the rules
   text is scanned for mechanical claims — skill/tool/armor/weapon/saving-throw
   proficiencies, languages, expertise, ability increases, spells learned or
   always prepared, resistances, immunities, senses, speeds, hit points, AC,
   feats — and compared with what the compendium actually grants for that record
   (including grants defined on its subclass or class). Run twice: once
   skipping conditional sentences ("while raging", "until the spell ends"), once
   with them (`RELAX=1`) to catch claims hidden by that filter.
2. **Structural checks**: background completeness (two skills; 2024 backgrounds
   have ability scores, a feat, a tool and equipment), feat prerequisites and
   categories, "choose…" in text without a recorded choice, feats whose versions
   were unreachable, leftover markup, empty or duplicated features.
3. **Manual review** of every flagged record, reading its text: fixed, or
   recorded below as correct.
4. **Regression tests** in `crates/rules/src/tests.rs` for each class of fix,
   plus the content audit test (all references resolve, every choice satisfiable).

## Fixed

**Data errors or omissions in 5etools** (via `crates/importer/patches.json`):
Rune Shaper (all 13 runes → Comprehend Languages + half-PB runes), Mark of
Detection (Detect Poison and Disease not prepared), Genie Touched (missing
Magic Initiate feat), Gloom Stalker Umbral Sight and 2024 Shadow monk Darkvision
(no darkvision), Diamond Soul / Disciplined Survivor (all saves), Purity of
Body, Divine Health, Nature's Ward, Beguiling Defenses, Unshakable Bravery,
Saint of Forge and Fire, Wind Soul, Fiery Soul (immunities), Guarded Mind,
Inured to Undeath, Necrotic Husk, Psychic Defenses, Oceanic Soul, Soul of the
Forge, Heart of the Storm, Fire in the Veins, Avatar of Battle, Radiant Soul
(resistances), 2024 Feral Senses, Shadow sorcerer (2014, RHW), Ascendant
Aspect, Demon Hunter (senses), Alchemist, Cartographer, Fey Wanderer, Horizon
Walker, Transmutation (free-cast spells), and proficiency/skill/language grants
for Death, Arcana (SCAG, AU), PSA domains, Knowledge (FRHoF), Purple Dragon
Knight, Banneret, Kensei, Ascendant Dragon, Giant, Drakewarden, EFA artificer
subclasses, Reanimator, Noble Genies, Bladesinger (FRHoF), Enchanter (AU),
Mystic subclasses, Mystic Brews, Elemental Adept damage type.

**Missing mechanics** (engine):
- Speeds: bonuses and granted speeds (Mobile, Speedy, Athlete, Boon of Speed,
  Roving 2014/2024, Superior Mobility, Aura of Alacrity, Gift of the Sea,
  Aquatic Affinity, Aspect of the Wilds, ...).
- AC: data-driven unarmored formulas (Barbarian, Monk, Dance bard, Draconic
  Resilience) — previously keyed off class names, so reskinned classes lost
  them; species natural armor (Tortle, Lizardfolk, Loxodon, Autognome,
  Thri-kreen, Locathah, Warforged, ...); conditional AC bonuses (Soul of the
  Forge); Medium Armor Master's +3 Dexterity cap.
- Darkvision that stacks with existing darkvision.
- Hit points per class level (Draconic Resilience, Immortal Durability).
- Tool expertise (2014 Rogue: thieves' tools).
- Feats with versions offer them (Strike of the Giants).
- Background features with prefixes ("Baldur's Gate Feature: …").

## Reviewed and correct as-is

These are flagged by the checker but are not character-sheet facts:

- **Conditional or temporary**: Rage, Bladesong, Rage of the Gods, Stormborn,
  Revelation in Flesh, Umbral Form, Exalted Champion, Form of Dread benefits
  (Superior Dread, Spirit Projection), Noble Scion, Crown of Spellfire, Wings
  Unfurled, Otherworldly Wings, Ghost Walk, Elemental Epitome, Dread Ambusher,
  Twinkling Constellations, Writhing Tide, Psi-Powered Leap, Adjust Density,
  Drunken Technique, Charger, Vampiric Exultation, Soul of the Storm Giant,
  Primordial Bond, Frozen Haunt, Lunar Empowerment, Superior Defense,
  Mortal Bulwark, Pet Specialization, Dispersion, Dragon Wings.
- **Re-chosen during play**: Bestial Soul, Power of the Wilds, Planar
  Wanderer, The Third Eye, Strength of Mind, Transmuter's Stone (also usable by
  another creature), Manifest Mind.
- **"If you already have X, choose another"**: the main grant is present
  (artificer tool proficiencies, Elegant Courtier, Iron Mind, Mystic Brews).
- **Examples, recommendations or focus rules in the text**: Spellcasting
  examples, "Boon of … is recommended", Expertise suggestions, Jack of All
  Trades, Actor, Heavy Armor Master, Artificer tools as a spellcasting focus,
  Reanimator tools as a focus, Wild Companion, Power Surge, Blade Flourish,
  Tales/Spirits from Beyond, Experimental Elixir.
- **Granted elsewhere**: Epic Boon and Fighting Style (class-level feat
  progressions), Fast/Unarmored Movement and Roving (computed on the sheet).

## Known limits

- Effects that only matter in play (uses per rest, damage riders, reactions)
  remain text on the sheet.
- "Timeless" (Hollow Warden, Ancient Might) is ambiguous between a permanent
  and a transformed benefit and is left as text.
- Simic Hybrid's carapace AC (a customization option) is left as text.
