# 5e Builder

A D&D 5e character builder, manager and printable character sheet, written in Rust.

- **Creation wizard** following the 2024 Player's Handbook. Older (legacy) species, backgrounds, subclasses and feats can be used too, following the 2024 rules for older material: legacy backgrounds get ability score increases and an origin feat; legacy species ability bonuses are optional; older subclasses are taken at level 3 on the 2024 class and gain any level 1–2 features then; where something was reprinted in 2024 (classes, feats) the reprint is used.
- **Selections, not menus**: wherever a feature says to choose (maneuvers, invocations, totem spirits, ancestries, artificer plans…), the sheet shows what was chosen.
- **Step order that respects prerequisites**: Class → Abilities → Background → Species → Feats → Spells → Equipment → Details. Base scores are set before any feat is picked, and class-level feats (ASI or feat, fighting style, epic boon) get their own step after every ability adjustment, so prerequisites like "Strength 13" use the real scores.
- **See everything before choosing**: every option at every step expands to its full text: classes (all features by level), species and lineages, backgrounds (everything they give, including exactly how they affect spells), subclasses, feats, spells (with casting details), skills, tools, languages, weapons and masteries, equipment packages and items. Classes, species and backgrounds have search and filters too.
- **Feat finder**: filter feats by category, ability increase, what they grant (spells, skills, expertise, resistances…), prerequisite, edition and book, or show only feats you can take now. Cards show category, ability increase, prerequisite and a one-line summary; click a card to read the full text, use the round button to select.
- **Custom step**: before Details, add anything the rules as written don't allow (house rules, DM rewards, homebrew): ability adjustments, proficiencies, languages, any feat or spell, your own features, bonuses to HP, AC or speed. It's applied to every number and printed on the sheet.
- **Every choice is tracked**: skills, tools, languages, ability score increases, feats and their sub-choices, fighting styles, invocations, weapon masteries, expertise, subclasses, spells, starting equipment. The wizard shows what is still missing on each step.
- **Levels 1–20 and multiclassing**, with average or rolled hit points.
- **Page one for the table**: a detailed Weapons & Damage Cantrips table (ability, proficiency, range, attack or save DC, damage, type, and a line with properties and what your weapon mastery does), an Actions box with three columns (actions, bonus actions, reactions), Class Features and Species Traits & Feats side by side, and languages with the other proficiencies.
- **Printable sheet**: a faithful replica of the official two-page 2024 character sheet layout, filled in, plus optional pages with full feature and spell text and a continuation page for anything that doesn't fit. Letter or A4 (on A4 the bottom boxes grow so the margins stay even); use the browser's *Print → Save as PDF*.
- **Character manager**: characters live in the browser (localStorage); duplicate, delete, and import/export JSON.
- **Foundry VTT export**: "Export for Foundry VTT" (character menu, Review step, sheet toolbar) saves an actor file for the D&D 5e system. See [Foundry VTT](#foundry-vtt).
- **All content** from the 5etools data, with per-source toggles.
- **Streamlined**: "choose the rest for me" per step and "pick for me" per choice (skills that use your best abilities, bonuses into your class's primary ability), auto-assign scores for your class, 4d6 rolling, undo/redo (Ctrl+Z), "next unfinished", spell-level and feat-category filters, and "new this level" highlights when levelling up.

## Design

The interface and printed sheet follow research-backed guidelines in
[`docs/design-guidelines.md`](docs/design-guidelines.md) (NN/G wizard and form
research, GOV.UK error-summary pattern, Baymard validation studies, WCAG 2.2
AA, print CSS practice, and 2024 character-sheet conventions). Checks:

- `python3 -I tools/contrast.py` — every colour token pair meets WCAG AA in
  light and dark themes.
- axe-core scans of every screen (light, dark, phone) report no violations.
- Fonts (Inter, Alegreya, Alegreya Sans) are self-hosted under the SIL Open Font License
  (`crates/web/fonts`).

Content coverage is documented in [`docs/content-audit.md`](docs/content-audit.md).

## Layout

```
crates/
  compendium/  Normalized content model: species, classes, spells, items…
               Content is described as Grants (fixed benefits) and Choices.
  importer/    5etools JSON → compendium.json (resolves _copy/_versions,
               flattens {@tags}, parses proficiency/spell/equipment blocks).
               patches.json adds structured data for mechanics 5etools only
               describes in prose (e.g. Cleric "Protector", Wizard "Scholar").
  rules/       Pure rules engine: (Character, Compendium) → Build (choices,
               grants, spells, features) → Sheet (AC, HP, saves, attacks…).
  web/         Leptos (client-side) app: manager, wizard, sheet.
scripts/
  fetch-5etools.sh  Sparse-clones the pinned 5etools release (data/ only).
  build-web.sh      Builds the static site into crates/web/dist.
```

The `Character` only stores the player's decisions (species, background,
class levels, and selections keyed by a stable choice path). Everything
else is derived, so changing an earlier decision never leaves stale data:
selections nested under a changed option are dropped automatically.

## Building

Prerequisites: Rust (stable), the wasm target and a `wasm-bindgen-cli`
matching the version in `Cargo.lock`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

Fetch the data and generate the compendium (the 5etools data is not
committed to this repository):

```sh
scripts/fetch-5etools.sh
cargo run -p importer --release -- data/5etools/data crates/web/public/compendium.json
```

Build and serve:

```sh
scripts/build-web.sh            # or --dev for a faster debug build
python3 -m http.server -d crates/web/dist 8080
```

The output in `crates/web/dist` is a static site and can be hosted anywhere.

`compendium.json` is generated, not committed. `build-web.sh` rebuilds it
whenever the importer or content model has changed since it was written,
and the app shows a warning if it is given one from an older importer
(it would load, but without newer data such as spell damage).

## Tests

```sh
cargo test --workspace --exclude web
```

`cargo run -p rules --example selection_audit -- crates/web/public/compendium.json`
builds every class × subclass at level 20 and every species and reports how
feature text was cut down to the selections (and any menus left uncovered).

`cargo run -p rules --example sample_characters -- crates/web/public/compendium.json`
prints a few fully built characters (multiclass caster, level 20 fighter and
wizard, barbarian/warlock, monk, legacy ranger) in the app's storage format, for checking the printed sheet.

`cargo run -p rules --example play_audit -- crates/web/public/compendium.json 20 xphb`
lists, for every class and subclass at a level, the limited uses and the
action / bonus action / reaction lists page one will show.

`cargo run -p rules --example foundry_export -- crates/web/public/compendium.json chars.json out/`
writes the Foundry actor for each character in a storage-format file.
The Foundry tests check, for every 2024 class at levels 1, 7 and 20, that
Foundry's own hit point calculation from the export equals the sheet,
that ids are unique and stable, and that every spell and link is present.
`python3 -I tools/foundry_check.py <dnd5e checkout> out/*.foundry.json` checks
exported files against the dnd5e system's source: every id and type it uses
must exist in the system's configuration and every field in its data models.
Add `--reference actor.json` (an actor exported from Foundry) to also list
fields the export writes that the reference never uses.

The rules tests run against the generated `compendium.json` when present.
An audit test checks that every feat/spell/item reference resolves and every
choice offers enough options; the importer warns about patches that no
longer match the data. The other tests
build characters for every class (levels 1, 5 and 20), every species
and every background and check that each choice can be satisfied, plus
hand-checked sheets (wizard, fighter, legacy rogue, cleric/wizard multiclass).

## Foundry VTT

The export uses the data model of Foundry's **D&D 5e system 6.x** (written
and checked against dnd5e 6.1.2 on Foundry v14). To import:

1. In Foundry, create an actor of type *Character* (any name).
2. Right-click it in the Actors sidebar → **Import Data** → choose the
   `<name>.foundry.json` file → *Import*. The actor's data is replaced.

Exporting the same character again gives the same item ids, so importing a
newer export over the same actor updates it.

What comes across:

- **Actor**: ability scores and save proficiencies, skills (proficiency,
  expertise, half proficiency), tool proficiencies, speeds, senses, size,
  languages, armor and weapon proficiencies, weapon masteries, resistances
  and immunities, hit points, coins, XP, alignment and every Details field.
- **Items**: class (levels, hit die, spellcasting progression), subclass,
  species, background, each feature and feat (with full text and the level
  it was gained), spells, and inventory (weapons with damage and attack,
  armor and shields, tools, consumables, gear) with equipped/attuned state.
- **Spells** use Foundry's casting methods: prepared, always prepared,
  spellbook (not prepared), pact magic, at will, and limited free casts
  with their uses and recovery (e.g. 1/long rest). Each has an attack,
  save, damage or utility activity with its damage dice, so it rolls in
  Foundry. Healing spells get a heal activity (dice plus your modifier),
  cantrip damage scales with level, leveled spells scale per slot level as
  their text says, and area spells (cone, sphere, cylinder, line…) carry
  their template.

Foundry computes some numbers itself (AC, hit points, spell slots, save
DCs). The export is arranged so its result equals this app's sheet: hit
points per level go in each class's Hit Points advancement and any extra
(Tough, Dwarven Toughness, …) becomes a bonus, so levelling up in Foundry
keeps working. AC uses Foundry's formulas (armor, Unarmored Defense,
Draconic Resilience) and is overridden only when none matches. Proficiencies
Foundry has no id for are kept as its custom text.

Not included: class advancement tables (levelling up in Foundry adds hit
points but not new features: add those from Foundry's compendiums or
re-export), token art, and active effects. Damage that depends on choices
made in play, and healing amounts, are left for the player to roll from the
description.

## Known gaps

- Choices made *during play* (e.g. which damage type an ability uses each
  time) are not tracked. Features whose effects are only described in prose
  appear as text on the sheet; their numbers are not computed. AC, speed and HP handle the common cases (armor, shields,
  Unarmored Defense, Draconic Resilience, Fast/Unarmored Movement, Tough,
  Dwarven Toughness, Defense, Alert).
- Generic magic item variants (e.g. "+1 Longsword") are not generated yet;
  add them as custom items.
