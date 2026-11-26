//! Printable character sheet (US Letter / A4).
//!
//! Layout follows docs/design-guidelines.md §3: page 1 holds everything used
//! every turn (modifiers are the big numbers; volatile state gets writable
//! boxes), later pages hold features, spells, equipment and story.

use crate::app::logo;
use crate::editor::{icon, ordinal};
use crate::entries::Entries;
use crate::store::Store;
use compendium::{signed, Compendium, Entry};
use leptos::prelude::*;
use rules::build::FeatureKind;
use rules::sheet::{Proficiency, Sheet, SkillLine};
use rules::Character;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct Opts {
    feature_text: RwSignal<bool>,
    spell_text: RwSignal<bool>,
    skills_az: RwSignal<bool>,
    a4: RwSignal<bool>,
}

#[component]
pub fn SheetPage(id: String) -> impl IntoView {
    let comp = expect_context::<Arc<Compendium>>();
    let store = expect_context::<Store>();
    let Some(ch) = store.with_untracked(|s| s.iter().find(|c| c.id == id).cloned()) else {
        return view! {
            <main class="loading"><div class="loading-card"><h1>"Character not found"</h1><a class="button primary" href="#/">"Back to characters"</a></div></main>
        }
        .into_any();
    };
    let o = Opts {
        feature_text: RwSignal::new(true),
        spell_text: RwSignal::new(false),
        skills_az: RwSignal::new(false),
        a4: RwSignal::new(false),
    };
    let b = rules::build(&ch, &comp);
    let s = rules::sheet(&ch, &b, &comp);
    let pending = rules::issues(&ch, &b, &comp).len();
    let edit_href = format!("#/edit/{id}/review");
    let toggle = move |label: &'static str, sig: RwSignal<bool>| {
        view! {
            <label class="switch">
                <input type="checkbox" prop:checked=move || sig.get() on:change=move |ev| sig.set(event_target_checked(&ev)) />
                <span>{label}</span>
            </label>
        }
    };
    let (s1, s2, s3, s4) = (s.clone(), s.clone(), s.clone(), s.clone());
    let (c1, c2) = (ch.clone(), ch.clone());

    view! {
        <div class="sheet-view">
            <header class="sheet-toolbar no-print">
                <a class="brand" href="#/" aria-label="5e Builder — all characters">{logo()}</a>
                <a class="button ghost small" href="#/">{icon("back")}"Characters"</a>
                <a class="button ghost small" href=edit_href>"Edit"</a>
                {(pending > 0).then(|| view! { <span class="chip warn">{format!("{pending} unfinished")}</span> })}
                <span class="spacer"></span>
                <div class="switches" role="group" aria-label="Sheet options">
                    {toggle("Feature text", o.feature_text)}
                    {toggle("Spell descriptions", o.spell_text)}
                    {toggle("Skills A–Z", o.skills_az)}
                    {toggle("A4 paper", o.a4)}
                </div>
                <button class="button primary" on:click=move |_| { if let Some(w) = web_sys::window() { let _ = w.print(); } }>{icon("print")}"Print or save PDF"</button>
            </header>
            {move || view! {
                <style>{if o.a4.get() {
                    "@page { size: A4; margin: 0; } .sheet .page { width: 210mm; min-height: 297mm; }"
                } else {
                    "@page { size: letter; margin: 0; } .sheet .page { width: 8.5in; min-height: 11in; }"
                }}</style>
            }}
            <main class="sheet" id="main" aria-label=format!("Character sheet for {}", s.name)>
                {move || main_page(&c1, &s1, o.skills_az.get())}
                {move || features_page(&s2, o.feature_text.get())}
                {spells_page(&s3, o)}
                {story_page(&c2, &s4)}
            </main>
        </div>
    }
    .into_any()
}

/// A labelled value box ("Armor Class", "Initiative", ...).
fn stat(label: &'static str, value: String, class: &'static str) -> impl IntoView {
    view! {
        <div class=format!("s-stat {class}")>
            <span class="s-stat-value">{value}</span>
            <span class="s-label">{label}</span>
        </div>
    }
}

fn boxes(n: usize) -> String {
    "☐".repeat(n)
}

fn prof_mark(p: Proficiency) -> (&'static str, &'static str) {
    match p {
        Proficiency::Expertise => ("◆", "expertise"),
        Proficiency::Proficient => ("●", "proficient"),
        Proficiency::Half => ("◐", "half proficiency"),
        Proficiency::None => ("○", "not proficient"),
    }
}

fn skill_row(k: &SkillLine, show_ability: bool) -> impl IntoView {
    let (mark, label) = prof_mark(k.prof);
    view! {
        <li>
            <span class="mark" title=label>{mark}<span class="visually-hidden">{format!(" {label}")}</span></span>
            <span class="val">{signed(k.bonus)}</span>
            <span class="name">{k.skill.name()}{show_ability.then(|| view! { <span class="abbr">{k.skill.ability().abbr()}</span> })}</span>
        </li>
    }
}

fn main_page(ch: &Character, s: &Sheet, skills_az: bool) -> impl IntoView {
    let d = &ch.details;
    let abilities = s
        .abilities
        .iter()
        .map(|a| {
            let skills: Vec<&SkillLine> = s.skills.iter().filter(|k| k.skill.ability() == a.ability).collect();
            view! {
                <div class="ab">
                    <div class="ab-box">
                        <span class="ab-name">{a.ability.name()}</span>
                        <span class="ab-mod">{signed(a.modifier)}</span>
                        <span class="ab-score" aria-label=format!("score {}", a.score)>{a.score}</span>
                    </div>
                    <ul class="ab-checks">
                        <li class="save">
                            <span class="mark">{if a.save_prof { "●" } else { "○" }}<span class="visually-hidden">{if a.save_prof { " proficient" } else { "" }}</span></span>
                            <span class="val">{signed(a.save)}</span>
                            <span class="name">"Saving Throw"</span>
                        </li>
                        {(!skills_az).then(|| skills.into_iter().map(|k| skill_row(k, false)).collect_view())}
                    </ul>
                </div>
            }
        })
        .collect_view();
    let mut az: Vec<&SkillLine> = s.skills.iter().collect();
    az.sort_by_key(|k| k.skill.name());
    let az_panel = skills_az.then(|| {
        view! {
            <section class="s-panel s-skills-az" aria-label="Skills">
                <h2 class="s-h">"Skills"</h2>
                <ul class="ab-checks">{az.into_iter().map(|k| skill_row(k, true)).collect_view()}</ul>
            </section>
        }
    });

    let speed = {
        let mut extra = Vec::new();
        for (n, v) in [("Fly", s.speed.fly), ("Swim", s.speed.swim), ("Climb", s.speed.climb), ("Burrow", s.speed.burrow)] {
            if v > 0 {
                extra.push(format!("{n} {v}"));
            }
        }
        extra.join(" · ")
    };
    let attacks = s
        .attacks
        .iter()
        .map(|a| {
            view! {
                <tr>
                    <td class="strong">{a.name.clone()}</td>
                    <td class="c">{signed(a.bonus)}</td>
                    <td>{a.damage.clone()}</td>
                    <td class="small">{a.notes.clone()}</td>
                </tr>
            }
        })
        .collect_view();
    let line = |label: &'static str, items: &[String]| {
        (!items.is_empty()).then(|| view! { <p class="s-line"><span class="s-k">{label}</span>{items.join(", ")}</p> })
    };
    let senses = {
        let mut v = Vec::new();
        if s.darkvision > 0 {
            v.push(format!("Darkvision {} ft.", s.darkvision));
        }
        v.extend(s.senses.iter().cloned());
        v
    };
    let feature_names = |kind: FeatureKind| -> Vec<String> {
        s.features.iter().filter(|f| f.kind == kind).map(|f| f.feature.name.clone()).collect()
    };
    let class_f = feature_names(FeatureKind::Class);
    let species_f = feature_names(FeatureKind::Species);
    let feat_f: Vec<String> = feature_names(FeatureKind::Feat).into_iter().chain(feature_names(FeatureKind::Other)).collect();
    let casting = s.casting.first().cloned();

    view! {
        <section class="page p-main" aria-label="Page 1: core statistics">
            <header class="s-head">
                <div class="s-name">
                    <h1>{s.name.clone()}</h1>
                    <span class="s-label">"Character name"</span>
                </div>
                <dl class="s-meta">
                    <div class="wide"><dd>{s.class_line.clone()}</dd><dt class="s-label">"Class & level"</dt></div>
                    <div><dd>{s.species.clone()}</dd><dt class="s-label">"Species"</dt></div>
                    <div><dd>{s.background.clone()}</dd><dt class="s-label">"Background"</dt></div>
                    <div><dd>{d.player.clone()}</dd><dt class="s-label">"Player"</dt></div>
                    <div><dd>{d.alignment.clone()}</dd><dt class="s-label">"Alignment"</dt></div>
                    <div><dd>{if d.xp > 0 { d.xp.to_string() } else { String::new() }}</dd><dt class="s-label">"Experience"</dt></div>
                </dl>
            </header>

            <div class="s-body">
                <div class="s-left">
                    <div class="s-pb">{stat("Proficiency Bonus", signed(s.proficiency_bonus), "pb")}</div>
                    {abilities}
                    <div class="s-passives">
                        {stat("Passive Perception", s.passive_perception.to_string(), "")}
                        {stat("Passive Insight", s.passive_insight.to_string(), "")}
                        {stat("Passive Investigation", s.passive_investigation.to_string(), "")}
                    </div>
                </div>

                <div class="s-right">
                    <div class="s-vitals">
                        <div class="s-stat ac">
                            <span class="s-stat-value">{s.armor_class}</span>
                            <span class="s-label">"Armor Class"</span>
                        </div>
                        {stat("Initiative", signed(s.initiative), "")}
                        <div class="s-stat">
                            <span class="s-stat-value">{format!("{} ft.", s.speed.walk)}</span>
                            <span class="s-label">"Speed"</span>
                            {(!speed.is_empty()).then(|| view! { <span class="s-sub">{speed.clone()}</span> })}
                        </div>
                        {stat("Size", s.size.clone(), "")}
                        <div class="s-stat write">
                            <span class="s-stat-value box-mark" aria-hidden="true">"☐"</span>
                            <span class="s-label">"Heroic Inspiration"</span>
                        </div>
                    </div>
                    <p class="s-note">{format!("AC: {}", s.ac_note)}</p>

                    <div class="s-hp">
                        <div class="s-hp-max"><span class="s-label">"Hit Point Maximum"</span><span class="s-hp-value">{s.hp_max}</span></div>
                        <div class="s-write"><span class="s-label">"Current Hit Points"</span></div>
                        <div class="s-write small"><span class="s-label">"Temporary"</span></div>
                    </div>
                    <div class="s-trackers">
                        <div class="s-tracker">
                            <span class="s-label">"Hit Dice"</span>
                            <span class="s-tracker-value">{s.hit_dice.clone()}</span>
                            <span class="s-sub">{format!("Spent {}", boxes(s.level as usize))}</span>
                        </div>
                        <div class="s-tracker">
                            <span class="s-label">"Death Saves"</span>
                            <span class="s-sub">"Successes ○○○"</span>
                            <span class="s-sub">"Failures ○○○"</span>
                        </div>
                        <div class="s-tracker">
                            <span class="s-label">"Exhaustion"</span>
                            <span class="s-sub">"1☐ 2☐ 3☐ 4☐ 5☐ 6☐"</span>
                            <span class="s-label">"Conditions"</span>
                        </div>
                    </div>

                    <section class="s-panel" aria-label="Weapons and damage cantrips">
                        <h2 class="s-h">"Weapons & Damage Cantrips"</h2>
                        <table class="s-table">
                            <thead><tr><th>"Name"</th><th class="c">"Atk / DC"</th><th>"Damage & type"</th><th>"Notes"</th></tr></thead>
                            <tbody>
                                {attacks}
                                <tr class="blank"><td colspan="4"><span class="visually-hidden">"blank line"</span></td></tr>
                                <tr class="blank"><td colspan="4"><span class="visually-hidden">"blank line"</span></td></tr>
                            </tbody>
                        </table>
                    </section>

                    <div class="s-two">
                        <section class="s-panel" aria-label="Proficiencies and training">
                            <h2 class="s-h">"Training & Proficiencies"</h2>
                            {line("Armor", &s.armor_profs)}
                            {line("Weapons", &s.weapon_profs)}
                            {line("Tools", &s.tool_profs)}
                            {line("Languages", &s.languages)}
                        </section>
                        <section class="s-panel" aria-label="Senses and defenses">
                            <h2 class="s-h">"Senses & Defenses"</h2>
                            {line("Senses", &senses)}
                            {line("Resistances", &s.resistances)}
                            {line("Immunities", &s.immunities)}
                            {line("Condition immunities", &s.condition_immunities)}
                            {line("Vulnerabilities", &s.vulnerabilities)}
                            {casting.map(|c| view! {
                                <p class="s-line"><span class="s-k">"Spellcasting"</span>
                                    {format!("{} ({}) · save DC {} · attack {}", c.label, c.ability.abbr(), c.save_dc, signed(c.attack))}
                                </p>
                            })}
                        </section>
                    </div>

                    <section class="s-panel s-features-list" aria-label="Features summary">
                        <h2 class="s-h">"Features"<span class="s-h-note">"Full text on the next page"</span></h2>
                        {line("Class", &class_f)}
                        {line("Species", &species_f)}
                        {line("Feats", &feat_f)}
                    </section>
                    {az_panel}
                    <section class="s-panel s-notes" aria-label="Class resources and notes">
                        <h2 class="s-h">"Class Resources & Notes"<span class="s-h-note">"Uses per rest, conditions, reminders"</span></h2>
                        <div class="s-ruled"></div>
                    </section>
                </div>
            </div>
        </section>
    }
}

fn features_page(s: &Sheet, full: bool) -> impl IntoView {
    let groups = [
        (FeatureKind::Class, "Class Features"),
        (FeatureKind::Species, "Species Traits"),
        (FeatureKind::Feat, "Feats"),
        (FeatureKind::Background, "Background"),
        (FeatureKind::Other, "Other Features"),
    ];
    let sections = groups
        .into_iter()
        .filter_map(|(kind, title)| {
            let items: Vec<_> = s.features.iter().filter(|f| f.kind == kind).cloned().collect();
            if items.is_empty() {
                return None;
            }
            Some(view! {
                <section class="s-feature-group">
                    <h2 class="s-group-title">{title}</h2>
                    {items.into_iter().map(|f| {
                        let body: Vec<Entry> = if full { f.feature.body.clone() } else { vec![] };
                        view! {
                            <article class="s-feature">
                                <h3>{f.feature.name.clone()}<span class="s-origin">{f.origin.clone()}</span></h3>
                                <Entries entries=body />
                            </article>
                        }
                    }).collect_view()}
                </section>
            })
        })
        .collect_view();
    view! {
        <section class="page p-features" aria-label="Page 2: features and traits">
            <h2 class="s-page-title">{s.name.clone()}<span>"Features & Traits"</span></h2>
            <div class="s-columns">{sections}</div>
        </section>
    }
}

fn spells_page(s: &Sheet, o: Opts) -> impl IntoView {
    if s.spells.is_empty() && s.casting.is_empty() {
        return ().into_any();
    }
    let casting = s
        .casting
        .iter()
        .map(|c| {
            view! {
                <div class="s-cast">
                    <span class="s-cast-name">{c.label.clone()}</span>
                    {stat("Ability", c.ability.abbr().to_string(), "")}
                    {stat("Spell Save DC", c.save_dc.to_string(), "")}
                    {stat("Spell Attack", signed(c.attack), "")}
                </div>
            }
        })
        .collect_view();
    let slots = s
        .slots
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(i, n)| view! { <div class="s-slot"><span class="s-slot-level">{ordinal(i as u8 + 1)}</span><span class="s-slot-boxes" aria-label=format!("{n} slots")>{boxes(*n as usize)}</span></div> })
        .collect_view();
    let pact = s.pact_slots.map(|(n, l)| {
        view! { <div class="s-slot"><span class="s-slot-level">{format!("Pact ({})", ordinal(l))}</span><span class="s-slot-boxes" aria-label=format!("{n} pact slots")>{boxes(n as usize)}</span></div> }
    });
    let mut levels: Vec<u8> = s.spells.iter().map(|x| x.level).collect();
    levels.dedup();
    let spells = s.spells.clone();
    let tables = levels
        .into_iter()
        .map(|lvl| {
            let rows = spells
                .iter()
                .filter(|x| x.level == lvl)
                .map(|x| {
                    // Only spellbook entries can be unprepared; innate and always-prepared spells are available.
                    let prepared = x.status != "Book";
                    let tags = [x.concentration.then_some("C"), x.ritual.then_some("R")].into_iter().flatten().collect::<Vec<_>>().join(" ");
                    let note = match x.status.as_str() {
                        "" | "Prepared" | "Known" | "Book" => String::new(),
                        "Always" => "always prepared".into(),
                        other => other.to_string(),
                    };
                    view! {
                        <tr>
                            <td class="c prep">{if lvl == 0 { "" } else if prepared { "■" } else { "☐" }}</td>
                            <td class="strong">{x.name.clone()}</td>
                            <td title=x.time.clone()>{x.time.split(',').next().unwrap_or("").to_string()}</td>
                            <td>{x.range.clone()}</td>
                            <td class="c">{tags}</td>
                            <td>{x.duration.clone()}</td>
                            <td class="small">{x.components.clone()}</td>
                            <td class="small">{note}</td>
                        </tr>
                    }
                })
                .collect_view();
            view! {
                <section class="s-spell-level">
                    <h3 class="s-h">{if lvl == 0 { "Cantrips".to_string() } else { format!("Level {lvl}") }}</h3>
                    <table class="s-table spells">
                        <colgroup>
                            <col class="c-prep" /><col class="c-name" /><col class="c-time" /><col class="c-range" />
                            <col class="c-cr" /><col class="c-dur" /><col class="c-comp" /><col class="c-notes" />
                        </colgroup>
                        <thead><tr>
                            <th class="c">"Prep"</th><th>"Spell"</th><th>"Time"</th><th>"Range"</th>
                            <th class="c"><abbr title="Concentration / Ritual">"C/R"</abbr></th><th>"Duration"</th><th>"Components"</th><th>"Notes"</th>
                        </tr></thead>
                        <tbody>
                            {rows}
                            {(0..if lvl == 0 { 1 } else { 3 }).map(|_| view! { <tr class="blank"><td colspan="8"><span class="visually-hidden">"blank line"</span></td></tr> }).collect_view()}
                        </tbody>
                    </table>
                </section>
            }
        })
        .collect_view();
    let descriptions = move || {
        o.spell_text.get().then(|| {
            let items = spells
                .iter()
                .map(|x| {
                    let lvl = if x.level == 0 { format!("{} cantrip", x.school) } else { format!("Level {} {}", x.level, x.school) };
                    view! {
                        <article class="s-feature">
                            <h3>{x.name.clone()}<span class="s-origin">{lvl}</span></h3>
                            <p class="s-spell-meta">{format!("{} · {} · {} · {}", x.time, x.range, x.components, x.duration)}</p>
                            <Entries entries=x.body.clone() />
                        </article>
                    }
                })
                .collect_view();
            view! {
                <section class="page p-spell-text" aria-label="Spell descriptions">
                    <h2 class="s-page-title">"Spell Descriptions"</h2>
                    <div class="s-columns">{items}</div>
                </section>
            }
        })
    };
    view! {
        <section class="page p-spells" aria-label="Spellcasting">
            <h2 class="s-page-title">{s.name.clone()}<span>"Spellcasting"</span></h2>
            <div class="s-cast-row">{casting}</div>
            <div class="s-slots"><span class="s-label">"Spell slots — mark when expended"</span>{slots}{pact}</div>
            {tables}
            <p class="s-note">"■ prepared · ☐ in your spellbook, not prepared · C concentration · R ritual"</p>
        </section>
        {descriptions}
    }
    .into_any()
}

fn story_page(ch: &Character, s: &Sheet) -> impl IntoView {
    let d = &ch.details;
    let inv = s
        .inventory
        .iter()
        .map(|i| {
            view! {
                <tr>
                    <td class="c">{if i.equipped { "●" } else { "" }}</td>
                    <td>{i.name.clone()}{(!i.notes.is_empty()).then(|| view! { <span class="s-sub block">{i.notes.clone()}</span> })}</td>
                    <td class="c">{i.qty}</td>
                    <td class="c">{if i.weight > 0.0 { format!("{}", i.weight) } else { String::new() }}</td>
                </tr>
            }
        })
        .collect_view();
    let attuned: Vec<String> = s.inventory.iter().filter(|i| i.attuned).map(|i| i.name.clone()).collect();
    let attune_slots = (0..3).map(|i| view! { <li class="s-attune">{attuned.get(i).cloned().unwrap_or_default()}</li> }).collect_view();
    let c = &s.coins;
    let text = |label: &'static str, v: &str, tall: bool| {
        view! { <section class="s-text" class:tall=tall><h3 class="s-h">{label}</h3><p class="prewrap">{v.to_string()}</p></section> }
    };
    let look = [
        ("Age", &d.age),
        ("Height", &d.height),
        ("Weight", &d.weight),
        ("Eyes", &d.eyes),
        ("Hair", &d.hair),
        ("Skin", &d.skin),
        ("Pronouns", &d.pronouns),
        ("Faith", &d.faith),
    ]
    .iter()
    .map(|(l, v)| view! { <div><dd>{v.to_string()}</dd><dt class="s-label">{*l}</dt></div> })
    .collect_view();
    view! {
        <section class="page p-story" aria-label="Equipment and story">
            <h2 class="s-page-title">{s.name.clone()}<span>"Equipment & Story"</span></h2>
            <div class="s-story">
                <div>
                    <section class="s-panel">
                        <h3 class="s-h">"Equipment"</h3>
                        <table class="s-table">
                            <thead><tr><th class="c"><abbr title="Equipped">"Eq."</abbr></th><th>"Item"</th><th class="c">"Qty"</th><th class="c">"lb."</th></tr></thead>
                            <tbody>
                                {inv}
                                <tr class="blank"><td colspan="4"><span class="visually-hidden">"blank line"</span></td></tr>
                                <tr class="blank"><td colspan="4"><span class="visually-hidden">"blank line"</span></td></tr>
                                <tr class="blank"><td colspan="4"><span class="visually-hidden">"blank line"</span></td></tr>
                            </tbody>
                        </table>
                        <p class="s-note">{format!("Carried {:.1} lb. · Carrying capacity {} lb.", s.carried_weight, s.carry_capacity)}</p>
                    </section>
                    <section class="s-panel">
                        <h3 class="s-h">"Magic Item Attunement"</h3>
                        <ol class="s-attunements">{attune_slots}</ol>
                    </section>
                    <section class="s-panel">
                        <h3 class="s-h">"Coins"</h3>
                        <div class="s-coins">
                            {stat("CP", c.cp.to_string(), "")}{stat("SP", c.sp.to_string(), "")}{stat("EP", c.ep.to_string(), "")}{stat("GP", c.gp.to_string(), "")}{stat("PP", c.pp.to_string(), "")}
                        </div>
                    </section>
                </div>
                <div>
                    <section class="s-panel">
                        <h3 class="s-h">"Appearance"</h3>
                        <dl class="s-meta look">{look}</dl>
                        <p class="prewrap">{d.appearance.clone()}</p>
                    </section>
                    {text("Personality Traits", &d.personality, false)}
                    {text("Ideals", &d.ideals, false)}
                    {text("Bonds", &d.bonds, false)}
                    {text("Flaws", &d.flaws, false)}
                </div>
            </div>
            {text("Backstory", &d.backstory, true)}
            <div class="s-two">
                {text("Allies & Organizations", &d.allies, false)}
                {text("Notes", &d.notes, false)}
            </div>
        </section>
    }
}
