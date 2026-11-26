# 5e Builder

A D&D 5e character builder, manager and printable character sheet, written in Rust.

- **Creation wizard** following the 2024 Player's Handbook. Older (legacy) species, backgrounds, classes and subclasses can be used too and are adapted to the 2024 rules (legacy backgrounds get ability score increases and an origin feat; legacy species ability bonuses are optional).
- **Every choice is tracked**: skills, tools, languages, ability score increases, feats and their sub-choices, fighting styles, invocations, weapon masteries, expertise, subclasses, spells, starting equipment. The wizard shows what is still missing on each step.
- **Levels 1–20 and multiclassing**, with average or rolled hit points.
- **Printable sheet**: main page, features, spellcasting (with slot boxes) and spell text, equipment and story. Use the browser's *Print → Save as PDF*.
- **Character manager**: characters live in the browser (localStorage); duplicate, delete, and import/export JSON.
- **All content** from the 5etools data, with per-source toggles.
- **Streamlined**: "choose the rest for me" per step and "pick for me" per choice (skills that use your best abilities, bonuses into your class's primary ability), auto-assign scores for your class, 4d6 rolling, undo/redo (Ctrl+Z), "next unfinished", spell-level and feat-category filters, and "new this level" highlights when levelling up.

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

## Tests

```sh
cargo test --workspace --exclude web
```

The rules tests run against the generated `compendium.json` when present.
An audit test checks that every feat/spell/item reference resolves and every
choice offers enough options; the importer warns about patches that no
longer match the data. The other tests
build characters for every class (levels 1, 5 and 20), every species
and every background and check that each choice can be satisfied, plus
hand-checked sheets (wizard, fighter, legacy rogue, cleric/wizard multiclass).

## Known gaps

- Choices made *during play* (e.g. which damage type an ability uses each
  time) are not tracked. Features whose effects are only described in prose
  appear as text on the sheet; their numbers are not computed. AC, speed and HP handle the common cases (armor, shields,
  Unarmored Defense, Draconic Resilience, Fast/Unarmored Movement, Tough,
  Dwarven Toughness, Defense, Alert).
- Generic magic item variants (e.g. "+1 Longsword") are not generated yet;
  add them as custom items.
